use crate::win32::{ERROR_INVALID_PARAMETER, ERROR_NEGATIVE_SEEK};
use encoding_rs::SHIFT_JIS;
use std::collections::{BTreeMap, BTreeSet};

use crate::memory::GuestAddress;

use super::RuntimeStateError;

pub const RUNTIME_FILE_SEARCH_LIMIT: usize = 4096;
pub const RUNTIME_SOURCE_READ_SERVICE: &str = "storage.source-read";
pub const RUNTIME_SOURCE_READ_REQUEST_BYTES: usize = 20;
pub const RUNTIME_SOURCE_READ_RESPONSE_CONTROL_BYTES: usize = 28;
pub const RUNTIME_SOURCE_FILE_LIMIT: usize = 4096;
pub const RUNTIME_SOURCE_READ_MAX_BYTES: u32 = 2 * 1024 * 1024;
pub const RUNTIME_OPFS_READ_SERVICE: &str = "browser.opfs-read";
pub const RUNTIME_WRITABLE_FILE_MAX_IO_BYTES: u32 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSourceSnapshotManifest {
    pub mount: RuntimeSourceMountManifest,
    pub files: Vec<RuntimeSourceFileManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSourceMountManifest {
    pub source_id: u32,
    pub root_path_a: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSourceFileManifest {
    pub object_id: u32,
    pub relative_path_a: Vec<u8>,
    pub byte_length: u64,
    pub sha256: String,
}

#[derive(Debug)]
pub(crate) struct RuntimeSourceSnapshot {
    pub source_id: u32,
    pub files: BTreeMap<u32, RuntimeSourceFileDescriptor>,
    path_index: BTreeMap<Vec<u8>, u32>,
    // Case-folded lookup key -> original normalized spelling for FindFirstFileA.
    directories: BTreeMap<Vec<u8>, Vec<u8>>,
    root_identity: Vec<u8>,
}

#[derive(Debug)]
pub(crate) struct RuntimeSourceFileDescriptor {
    pub relative_path_a: Vec<u8>,
    pub byte_length: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RuntimeSourceFileId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeFileSearchId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFileSearchEntryKind {
    RegularFile,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFileSearchEntry {
    pub file_name_a: Vec<u8>,
    pub kind: RuntimeFileSearchEntryKind,
    pub byte_length: u64,
}

impl RuntimeFileSearchEntry {
    pub(super) fn encode_find_data_a(&self) -> Result<Vec<u8>, RuntimeStateError> {
        if self.file_name_a.is_empty()
            || self.file_name_a.len() >= 260
            || self.file_name_a.contains(&0)
        {
            return Err(RuntimeStateError::new(
                "file search result name does not fit WIN32_FIND_DATAA.cFileName",
            ));
        }
        let mut bytes = vec![0; 320];
        let attributes: u32 = match self.kind {
            RuntimeFileSearchEntryKind::RegularFile => 0x20,
            RuntimeFileSearchEntryKind::Directory => 0x30,
        };
        bytes[0..4].copy_from_slice(&attributes.to_le_bytes());
        bytes[28..32].copy_from_slice(&((self.byte_length >> 32) as u32).to_le_bytes());
        bytes[32..36].copy_from_slice(&(self.byte_length as u32).to_le_bytes());
        bytes[44..44 + self.file_name_a.len()].copy_from_slice(&self.file_name_a);
        Ok(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFileSearch {
    pub id: RuntimeFileSearchId,
    pub normalized_pattern_a: Vec<u8>,
    pub entries: Vec<RuntimeFileSearchEntry>,
    pub next_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeFileBacking {
    ImmutableSource { source_id: u32, object_id: u32 },
    Opfs { byte_length: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePathResolution {
    RegularFile,
    Directory,
    MissingFile,
    MissingPath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeSourceFile {
    pub id: RuntimeSourceFileId,
    pub backing: RuntimeFileBacking,
    pub normalized_path_a: Vec<u8>,
    pub cursor: u64,
    pub desired_access: u32,
    pub share_mode: u32,
    pub flags_and_attributes: u32,
}

#[derive(Debug)]
pub(crate) enum SourceOpenOutcome<T> {
    Opened(T),
    Missing,
    SharingViolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeSourceReadRequest {
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

#[derive(Debug)]
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
            return Err(RuntimeStateError::new(
                "source read response control must contain seven little-endian words",
            ));
        }
        let words: [u32; 7] = std::array::from_fn(|index| {
            let offset = index * 4;
            u32::from_le_bytes(
                control[offset..offset + 4]
                    .try_into()
                    .expect("four-byte source read response control chunk"),
            )
        });
        let requested_bytes = words[4];
        let transferred_bytes = words[5];
        let win32_error = words[6];
        if requested_bytes == 0 || requested_bytes > RUNTIME_SOURCE_READ_MAX_BYTES {
            return Err(RuntimeStateError::new(
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
            return Err(RuntimeStateError::new(
                "source read response payload does not match its result metadata",
            ));
        }
        Ok(Self {
            request_id,
            source_id: words[0],
            object_id: words[1],
            offset: u64::from(words[2]) | (u64::from(words[3]) << 32),
            requested_bytes,
            transferred_bytes,
            win32_error,
            bytes: payload.to_vec(),
        })
    }
}

// The selected directory spelling and its Rust-matched immediate children.
pub(super) type SourceSearchSelection = (Vec<u8>, Vec<(Vec<u8>, bool)>);
pub(super) type SourceSearchBatchEntry = (Vec<u8>, Option<(u64, String)>);

#[derive(Debug)]
pub(super) struct RuntimeFiles {
    snapshot: Option<RuntimeSourceSnapshot>,
    open_files: BTreeMap<RuntimeSourceFileId, RuntimeSourceFile>,
    pending_reads: BTreeMap<u64, RuntimePendingSourceRead>,
    searches: BTreeMap<RuntimeFileSearchId, RuntimeFileSearch>,
    next_file_id: u64,
    next_search_id: u64,
}

impl Default for RuntimeFiles {
    fn default() -> Self {
        Self {
            snapshot: None,
            open_files: BTreeMap::new(),
            pending_reads: BTreeMap::new(),
            searches: BTreeMap::new(),
            next_file_id: 1,
            next_search_id: 1,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct RuntimeSourceFileOpen {
    next_file_id: u64,
    file: RuntimeSourceFile,
}

impl RuntimeSourceFileOpen {
    pub(super) const fn file_id(&self) -> RuntimeSourceFileId {
        self.file.id
    }

    pub(super) fn confirmed_opfs(mut self, byte_length: u64) -> Self {
        debug_assert!(matches!(self.file.backing, RuntimeFileBacking::Opfs { .. }));
        self.file.backing = RuntimeFileBacking::Opfs { byte_length };
        self
    }
}

#[derive(Debug)]
struct RuntimePendingSourceRead {
    request: RuntimeSourceReadRequest,
    buffer: GuestAddress,
    bytes_read: GuestAddress,
}

#[derive(Debug)]
pub(super) struct RuntimeSourceReadCompletionPlan {
    request_id: u64,
    file_id: RuntimeSourceFileId,
    buffer: GuestAddress,
    bytes_read: GuestAddress,
    next_cursor: u64,
    service: &'static str,
}

impl RuntimeSourceReadCompletionPlan {
    pub(super) const fn buffer(&self) -> GuestAddress {
        self.buffer
    }

    pub(super) const fn bytes_read(&self) -> GuestAddress {
        self.bytes_read
    }

    pub(super) const fn service(&self) -> &'static str {
        self.service
    }
}

impl RuntimeFiles {
    pub(super) fn snapshot(&self) -> Option<&RuntimeSourceSnapshot> {
        self.snapshot.as_ref()
    }

    pub(super) fn resolve_path_kind(
        &self,
        requested_path: &[u8],
        current_directory: &[u8],
    ) -> Result<RuntimePathResolution, RuntimeStateError> {
        let normalized = resolve_path(requested_path, current_directory)?;
        let root_identity = self
            .snapshot
            .as_ref()
            .ok_or_else(|| {
                RuntimeStateError::new("runtime process has no selected source directory")
            })?
            .root_identity
            .as_slice();
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
                return Err(RuntimeStateError::new(
                    "source path index points to a missing object descriptor",
                ));
            }
            return Ok(RuntimePathResolution::RegularFile);
        }
        if self.registered_directory_exists(relative) {
            return Ok(RuntimePathResolution::Directory);
        }
        let components = normalize_components(relative, false)?;
        let parent = components[..components.len() - 1].join(&b"\\"[..]);
        Ok(
            if parent.is_empty() || self.registered_directory_exists(&parent) {
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
        self.snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.directories.contains_key(relative)
                || snapshot.path_index.keys().any(|path| is_descendant(path))
                || snapshot.directories.keys().any(|path| is_descendant(path))
        })
    }

    fn enumerate_file_search_entries(
        &self,
        pattern: &ResolvedFileSearchPattern,
    ) -> Result<Option<Vec<RuntimeFileSearchEntry>>, RuntimeStateError> {
        let root_identity = self
            .snapshot
            .as_ref()
            .ok_or_else(|| {
                RuntimeStateError::new("file search requires a selected source directory")
            })?
            .root_identity
            .as_slice();
        let directory_relative = if pattern.directory_identity == root_identity {
            &[][..]
        } else if pattern.directory_identity.starts_with(root_identity)
            && pattern.directory_identity.get(root_identity.len()) == Some(&b'\\')
        {
            &pattern.directory_identity[root_identity.len() + 1..]
        } else {
            return Ok(None);
        };
        if !directory_relative.is_empty() && !self.registered_directory_exists(directory_relative) {
            return Ok(None);
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
            for path in snapshot.directories.values() {
                insert_path(path, RuntimeFileSearchEntryKind::Directory, 0);
            }
        }

        let mut entries = Vec::new();
        for candidate in candidates.into_values() {
            if !windows_932_wildcard_matches(&pattern.file_name_pattern, &candidate.file_name_a)? {
                continue;
            }
            if entries.len() == RUNTIME_FILE_SEARCH_LIMIT {
                return Err(RuntimeStateError::new(
                    "file search result count exceeds the policy bound",
                ));
            }
            entries.push(candidate);
        }
        Ok(Some(entries))
    }

    pub(super) fn open_file(
        &self,
        id: RuntimeSourceFileId,
    ) -> Result<&RuntimeSourceFile, RuntimeStateError> {
        self.open_files
            .get(&id)
            .ok_or_else(|| RuntimeStateError::new("logical source file identity is not open"))
    }

    pub(super) fn file_search(
        &self,
        id: RuntimeFileSearchId,
    ) -> Result<&RuntimeFileSearch, RuntimeStateError> {
        self.searches
            .get(&id)
            .ok_or_else(|| RuntimeStateError::new("logical file search identity is not open"))
    }

    pub(super) fn new_file_search(
        &self,
        requested_pattern: &[u8],
        current_directory: &[u8],
    ) -> Result<Result<Option<RuntimeFileSearch>, u32>, RuntimeStateError> {
        if self.searches.len() >= RUNTIME_FILE_SEARCH_LIMIT {
            return Err(RuntimeStateError::new(
                "logical file search count exceeds the runtime bound",
            ));
        }
        let pattern = resolve_file_search_pattern(requested_pattern, current_directory)?;
        let Some(entries) = self.enumerate_file_search_entries(&pattern)? else {
            return Ok(Err(3)); // ERROR_PATH_NOT_FOUND
        };
        if entries.is_empty() {
            return Ok(Ok(None));
        }
        let id = RuntimeFileSearchId(self.next_search_id);
        self.next_search_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::new("logical file search identifier space is exhausted")
        })?;
        if self.searches.contains_key(&id) {
            return Err(RuntimeStateError::new(
                "file search allocator selected a live identity",
            ));
        }
        Ok(Ok(Some(RuntimeFileSearch {
            id,
            normalized_pattern_a: pattern.normalized_pattern_a,
            entries,
            next_index: 1,
        })))
    }

    // Published only after the aggregate has validated the handle and guest output.
    pub(super) fn insert_file_search(&mut self, search: RuntimeFileSearch) {
        self.next_search_id += 1;
        self.searches.insert(search.id, search);
    }

    pub(super) fn next_file_search_entry(
        &self,
        id: RuntimeFileSearchId,
    ) -> Result<Option<&RuntimeFileSearchEntry>, RuntimeStateError> {
        let search = self.file_search(id)?;
        if search.next_index > search.entries.len() {
            return Err(RuntimeStateError::new(
                "file search cursor is outside its result list",
            ));
        }
        Ok(search.entries.get(search.next_index))
    }

    // A successful guest write is the only intervening operation after entry validation.
    pub(super) fn advance_file_search(&mut self, id: RuntimeFileSearchId) {
        self.searches
            .get_mut(&id)
            .expect("validated file search cursor")
            .next_index += 1;
    }

    // The aggregate checks the search and its handle before either owner changes.
    pub(super) fn retire_file_search(&mut self, id: RuntimeFileSearchId) {
        self.searches
            .remove(&id)
            .expect("validated file search retirement");
    }

    pub(super) fn register_snapshot(
        &mut self,
        manifest: RuntimeSourceSnapshotManifest,
    ) -> Result<(), RuntimeStateError> {
        if self.snapshot.is_some()
            || !self.open_files.is_empty()
            || !self.pending_reads.is_empty()
            || !self.searches.is_empty()
        {
            return Err(RuntimeStateError::new(
                "runtime source snapshot registration is process-static",
            ));
        }
        if manifest.mount.source_id == 0
            || manifest.files.is_empty()
            || manifest.files.len() > RUNTIME_SOURCE_FILE_LIMIT
        {
            return Err(RuntimeStateError::new(
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
                return Err(RuntimeStateError::new(
                    "source file manifest identity, size, or hash is invalid",
                ));
            }
            let relative = normalize_relative_path(&file.relative_path_a)?;
            let identity = windows_932_ascii_fold(&relative);
            if files.contains_key(&file.object_id) || path_index.contains_key(&identity) {
                return Err(RuntimeStateError::new(
                    "source file object or case-insensitive path identity is duplicated",
                ));
            }

            path_index.insert(identity, file.object_id);
            files.insert(
                file.object_id,
                RuntimeSourceFileDescriptor {
                    relative_path_a: relative,
                    byte_length: file.byte_length,
                },
            );
        }
        let next = RuntimeSourceSnapshot {
            source_id: manifest.mount.source_id,
            files,
            path_index,
            directories: BTreeMap::new(),
            root_identity: root.identity,
        };
        self.snapshot = Some(next);
        Ok(())
    }

    pub(super) fn source_lookup_relative_path(
        &self,
        requested_path: &[u8],
        current_directory: &[u8],
    ) -> Result<Option<Vec<u8>>, RuntimeStateError> {
        let normalized = resolve_path(requested_path, current_directory)?;
        let snapshot = self
            .snapshot
            .as_ref()
            .ok_or_else(|| RuntimeStateError::new("source lookup requires a selected directory"))?;
        let relative = relative_to_mount(&normalized.identity, &snapshot.root_identity)?;
        if snapshot.path_index.contains_key(relative) || self.registered_directory_exists(relative)
        {
            return Ok(None);
        }
        let relative_start = snapshot.root_identity.len() + 1;
        let relative_bytes = normalized
            .bytes_with_nul
            .get(relative_start..normalized.bytes_with_nul.len() - 1)
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "source lookup path does not lie under the selected directory",
                )
            })?;
        Ok(Some(normalize_relative_path(relative_bytes)?))
    }

    // Windows failures are returned directly here. The browser receives only a
    // decoded relative path, never a DOS drive or a path outside this mount.
    pub(super) fn opfs_relative_path(
        &self,
        requested: &[u8],
        current_directory: &[u8],
    ) -> Result<String, u32> {
        let body = nul_terminated_body(requested).map_err(|_| 123_u32)?;
        // Root-relative and UNC paths are not relative to this game's tree.
        if body.first().is_some_and(|byte| is_separator(*byte)) {
            return Err(3);
        }
        let normalized = resolve_path(requested, current_directory).map_err(|_| 123_u32)?;
        let snapshot = self.snapshot.as_ref().ok_or(3_u32)?;
        if normalized.identity == snapshot.root_identity {
            return Err(183);
        }
        let identity =
            relative_to_mount(&normalized.identity, &snapshot.root_identity).map_err(|_| 3_u32)?;
        let components = normalize_components(identity, false).map_err(|_| 123_u32)?;
        let mut prefix = Vec::new();
        for component in components.iter().take(components.len() - 1) {
            if !prefix.is_empty() {
                prefix.push(b'\\');
            }
            prefix.extend_from_slice(component);
            if snapshot.path_index.contains_key(&prefix) {
                return Err(3); // An immutable source file cannot contain an OPFS child.
            }
        }
        let relative = &normalized.bytes_with_nul
            [snapshot.root_identity.len() + 1..normalized.bytes_with_nul.len() - 1];
        let decoded = encoding_rs::SHIFT_JIS
            .decode_without_bom_handling_and_without_replacement(relative)
            .ok_or(123_u32)?;
        Ok(decoded.replace('\\', "/"))
    }

    // The source view may contain a parent directory that OPFS has not yet
    // mirrored. Only those checked source parents may be created by the host;
    // an arbitrary missing parent must still fail CreateDirectoryA.
    pub(super) fn opfs_create_directory_path(
        &self,
        requested: &[u8],
        current_directory: &[u8],
    ) -> Result<(String, u8), u32> {
        let relative_path = self.opfs_relative_path(requested, current_directory)?;
        let normalized = resolve_path(requested, current_directory).map_err(|_| 123_u32)?;
        let snapshot = self.snapshot.as_ref().ok_or(3_u32)?;
        let relative =
            relative_to_mount(&normalized.identity, &snapshot.root_identity).map_err(|_| 3_u32)?;
        if snapshot.path_index.contains_key(relative) || self.registered_directory_exists(relative)
        {
            return Err(183);
        }
        let mut prefix = Vec::new();
        let mut source_parents = 0_u8;
        let components = normalize_components(relative, false).map_err(|_| 123_u32)?;
        for (index, component) in components.iter().take(components.len() - 1).enumerate() {
            if !prefix.is_empty() {
                prefix.push(b'\\');
            }
            prefix.extend_from_slice(component);
            if snapshot.path_index.contains_key(&prefix) {
                return Err(3); // A source file cannot be a parent directory.
            }
            if self.registered_directory_exists(&prefix) {
                source_parents = u8::try_from(index + 1).map_err(|_| 123_u32)?;
            } else {
                break;
            }
        }
        Ok((relative_path, source_parents))
    }

    pub(super) fn opfs_delete_path(
        &self,
        requested: &[u8],
        current_directory: &[u8],
    ) -> Result<(String, u8), u32> {
        let relative_path = self.opfs_relative_path(requested, current_directory)?;
        let normalized = resolve_path(requested, current_directory).map_err(|_| 123_u32)?;
        let snapshot = self.snapshot.as_ref().ok_or(3_u32)?;
        let relative =
            relative_to_mount(&normalized.identity, &snapshot.root_identity).map_err(|_| 3_u32)?;
        if snapshot.path_index.contains_key(relative) || self.registered_directory_exists(relative)
        {
            return Err(5); // Immutable source or directory.
        }
        if self.open_files.values().any(|file| {
            resolve_path(&file.normalized_path_a, current_directory)
                .is_ok_and(|open| open.identity == normalized.identity)
        }) {
            return Err(32); // An open file cannot be removed while its handle is live.
        }
        let components = normalize_components(relative, false).map_err(|_| 123_u32)?;
        let parent = components[..components.len() - 1].join(&b"\\"[..]);
        let missing_error = if parent.is_empty() || self.registered_directory_exists(&parent) {
            2 // OPFS may lack a parent that exists in the immutable source view.
        } else {
            3
        };
        Ok((relative_path, missing_error))
    }

    // The browser enumerates a directory capability, but matching and DOS path
    // identity remain Rust decisions. Only a wildcard in the final component
    // can trigger a directory listing.
    pub(super) fn source_search_directory(
        &self,
        requested: &[u8],
        current_directory: &[u8],
    ) -> Result<Option<Vec<u8>>, RuntimeStateError> {
        let pattern = resolve_file_search_pattern(requested, current_directory)?;
        if !pattern
            .file_name_pattern
            .iter()
            .any(|byte| matches!(byte, b'*' | b'?'))
        {
            return Ok(None);
        }
        let snapshot = self
            .snapshot
            .as_ref()
            .ok_or_else(|| RuntimeStateError::new("source search requires a selected directory"))?;
        if pattern.directory_identity == snapshot.root_identity {
            return Ok(Some(Vec::new()));
        }
        relative_to_mount(&pattern.directory_identity, &snapshot.root_identity)?;
        // The spelling used to inspect the browser directory must still pass
        // the same Windows-932 component validation as a literal lookup.
        let prefix = &pattern.normalized_pattern_a
            [..pattern.normalized_pattern_a.len() - 1 - pattern.file_name_pattern.len()];
        let start = snapshot.root_identity.len() + 1;
        Ok(Some(normalize_relative_path(
            &prefix[start..prefix.len() - 1],
        )?))
    }

    pub(super) fn source_search_matches(
        &self,
        requested: &[u8],
        current_directory: &[u8],
        actual_segments: &[String],
        names: &[(String, bool)],
    ) -> Result<SourceSearchSelection, RuntimeStateError> {
        if names.len() > RUNTIME_SOURCE_FILE_LIMIT {
            return Err(RuntimeStateError::new(
                "browser directory exceeds the bounded source file count",
            ));
        }
        let pattern = resolve_file_search_pattern(requested, current_directory)?;
        let parent = self
            .source_search_directory(requested, current_directory)?
            .ok_or_else(|| RuntimeStateError::new("source search requires a wildcard"))?;
        let mut actual = Vec::new();
        if actual_segments.len() > 256 {
            return Err(RuntimeStateError::new(
                "browser directory has too many components",
            ));
        }
        for segment in actual_segments {
            let (encoded, _, had_errors) = SHIFT_JIS.encode(segment);
            if had_errors || validate_component(&encoded).is_err() {
                return Err(RuntimeStateError::new(
                    "browser directory name is not a Windows-932 component",
                ));
            }
            if !actual.is_empty() {
                actual.push(b'\\');
            }
            actual.extend_from_slice(&encoded);
        }
        if windows_932_ascii_fold(&actual) != windows_932_ascii_fold(&parent) {
            return Err(RuntimeStateError::new(
                "browser directory does not match the requested Windows path",
            ));
        }
        let snapshot = self.snapshot.as_ref().expect("validated source mount");
        let mut selected = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for (name, is_file) in names {
            let (encoded, _, had_errors) = SHIFT_JIS.encode(name);
            if had_errors || validate_component(&encoded).is_err() {
                continue; // This browser name cannot be expressed as a DOS filename.
            }
            let name_bytes = encoded.as_ref();
            if !seen.insert(windows_932_ascii_fold(name_bytes)) {
                return Err(RuntimeStateError::new(
                    "browser directory contains colliding Windows path names",
                ));
            }
            if !windows_932_wildcard_matches(&pattern.file_name_pattern, name_bytes)? {
                continue;
            }
            let mut path = actual.clone();
            if !path.is_empty() {
                path.push(b'\\');
            }
            path.extend_from_slice(name_bytes);
            if path.len() > 260 {
                continue;
            }
            let identity = windows_932_ascii_fold(&path);
            if snapshot.path_index.contains_key(&identity)
                || snapshot.directories.contains_key(&identity)
            {
                continue;
            }
            if selected.insert(identity, (path, *is_file)).is_some() {
                return Err(RuntimeStateError::new(
                    "browser directory contains colliding Windows path names",
                ));
            }
        }
        Ok((actual, selected.into_values().collect()))
    }

    pub(super) fn register_source_search_batch(
        &mut self,
        parent: &[u8],
        entries: Vec<SourceSearchBatchEntry>,
    ) -> Result<Vec<(usize, u32)>, RuntimeStateError> {
        if entries.len() > RUNTIME_SOURCE_FILE_LIMIT {
            return Err(RuntimeStateError::new("too many source search entries"));
        }
        let snapshot = self.snapshot.as_mut().ok_or_else(|| {
            RuntimeStateError::new("source search registration requires a selected directory")
        })?;
        let mut next_id = snapshot.files.keys().next_back().copied().unwrap_or(0);
        let mut files = BTreeMap::new();
        let mut paths = BTreeMap::new();
        let mut directories = BTreeMap::new();
        let mut registered = Vec::new();
        let parent = (!parent.is_empty()).then(|| (None, (parent.to_vec(), None)));
        for (index, (path, info)) in parent.into_iter().chain(
            entries
                .into_iter()
                .enumerate()
                .map(|(index, entry)| (Some(index), entry)),
        ) {
            if let Some((byte_length, sha256)) = info {
                next_id = next_id
                    .checked_add(1)
                    .ok_or_else(|| RuntimeStateError::new("source file identities exhausted"))?;
                if byte_length > u64::from(u32::MAX) || !is_sha256(&sha256) {
                    return Err(RuntimeStateError::new(
                        "source file length or SHA-256 is invalid",
                    ));
                }
                if snapshot.files.len() + files.len() >= RUNTIME_SOURCE_FILE_LIMIT {
                    return Err(RuntimeStateError::new(
                        "source directory exceeds the number of registered files",
                    ));
                }
                let path = normalize_relative_path(&path)?;
                let identity = windows_932_ascii_fold(&path);
                if snapshot.path_index.contains_key(&identity)
                    || paths.contains_key(&identity)
                    || snapshot.directories.contains_key(&identity)
                    || directories.contains_key(&identity)
                {
                    return Err(RuntimeStateError::new(
                        "source Windows path is already registered",
                    ));
                }

                paths.insert(identity, next_id);
                files.insert(
                    next_id,
                    RuntimeSourceFileDescriptor {
                        relative_path_a: path,
                        byte_length,
                    },
                );
                registered.push((index.expect("only entries can register files"), next_id));
            } else {
                let path = normalize_relative_path(&path)?;
                let identity = windows_932_ascii_fold(&path);
                if snapshot.path_index.contains_key(&identity) || paths.contains_key(&identity) {
                    return Err(RuntimeStateError::new(
                        "source directory conflicts with a registered file",
                    ));
                }
                if !snapshot.directories.contains_key(&identity) {
                    directories.entry(identity).or_insert(path);
                }
            }
        }
        snapshot.files.extend(files);
        snapshot.path_index.extend(paths);
        snapshot.directories.extend(directories);
        Ok(registered)
    }

    pub(super) fn register_source_directory(
        &mut self,
        relative_path_a: &[u8],
    ) -> Result<(), RuntimeStateError> {
        let normalized = normalize_relative_path(relative_path_a)?;
        let identity = windows_932_ascii_fold(&normalized);
        let snapshot = self.snapshot.as_mut().ok_or_else(|| {
            RuntimeStateError::new("source directory lookup requires a selected directory")
        })?;
        if snapshot.path_index.contains_key(&identity) {
            return Err(RuntimeStateError::new(
                "source directory conflicts with a registered file",
            ));
        }
        if snapshot.directories.contains_key(&identity) {
            return Ok(());
        }
        snapshot.directories.insert(identity, normalized);
        Ok(())
    }

    pub(super) fn register_source_file(
        &mut self,
        file: RuntimeSourceFileManifest,
    ) -> Result<(), RuntimeStateError> {
        let snapshot = self.snapshot.as_mut().ok_or_else(|| {
            RuntimeStateError::new("source file registration requires a mounted source directory")
        })?;
        if file.object_id == 0 || file.byte_length > u64::from(u32::MAX) || !is_sha256(&file.sha256)
        {
            return Err(RuntimeStateError::new(
                "source file identity, length, or SHA-256 is invalid",
            ));
        }
        if snapshot.files.len() >= RUNTIME_SOURCE_FILE_LIMIT {
            return Err(RuntimeStateError::new(
                "source directory exceeds the number of registered files",
            ));
        }
        let path = normalize_relative_path(&file.relative_path_a)?;
        let identity = windows_932_ascii_fold(&path);
        if snapshot.files.contains_key(&file.object_id)
            || snapshot.path_index.contains_key(&identity)
            || snapshot.directories.contains_key(&identity)
        {
            return Err(RuntimeStateError::new(
                "source file object or Windows path is already registered",
            ));
        }

        snapshot.path_index.insert(identity, file.object_id);
        snapshot.files.insert(
            file.object_id,
            RuntimeSourceFileDescriptor {
                relative_path_a: path,
                byte_length: file.byte_length,
            },
        );
        Ok(())
    }

    pub(super) fn prepare_open_with_disposition(
        &self,
        requested_path: &[u8],
        current_directory: &[u8],
        desired_access: u32,
        share_mode: u32,
        creation_disposition: u32,
        flags_and_attributes: u32,
    ) -> Result<SourceOpenOutcome<RuntimeSourceFileOpen>, RuntimeStateError> {
        let normalized = resolve_path(requested_path, current_directory)?;
        let snapshot = self.snapshot.as_ref().ok_or_else(|| {
            RuntimeStateError::new("runtime process has no selected source directory")
        })?;
        let relative = relative_to_mount(&normalized.identity, &snapshot.root_identity)?;
        if self.open_files.len() >= RUNTIME_SOURCE_FILE_LIMIT {
            return Err(RuntimeStateError::new(
                "logical file count exceeds the runtime bound",
            ));
        }
        let Some(object_id) = snapshot.path_index.get(relative).copied() else {
            return Ok(SourceOpenOutcome::Missing);
        };
        if creation_disposition != 3 || desired_access & 0x4000_0000 != 0 {
            return Err(RuntimeStateError::new(
                "immutable source files cannot be created, truncated, or opened for write",
            ));
        }
        if !snapshot.files.contains_key(&object_id) {
            return Err(RuntimeStateError::new(
                "source path index points to a missing object descriptor",
            ));
        }
        let backing = RuntimeFileBacking::ImmutableSource {
            source_id: snapshot.source_id,
            object_id,
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
                return Ok(SourceOpenOutcome::SharingViolation);
            }
        }
        let id = RuntimeSourceFileId(self.next_file_id);
        let next_file_id = self.next_file_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::new("logical source file identifier space is exhausted")
        })?;
        Ok(SourceOpenOutcome::Opened(RuntimeSourceFileOpen {
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
        }))
    }

    // This open leaves Rust while OPFS checks/creates the physical file. The
    // candidate handle and file ID are reserved by the aggregate before that
    // browser work; no logical file is published until durable completion.
    pub(super) fn prepare_opfs_open(
        &self,
        requested_path: &[u8],
        current_directory: &[u8],
        desired_access: u32,
        share_mode: u32,
        flags_and_attributes: u32,
    ) -> Result<(RuntimeSourceFileOpen, String, u8, u8), u32> {
        let (relative_path, source_parents) = self
            .opfs_create_directory_path(requested_path, current_directory)
            .map_err(|error| if error == 183 { 5 } else { error })?;
        let source_missing = match self.resolve_path_kind(requested_path, current_directory) {
            Ok(RuntimePathResolution::MissingFile) => 2,
            Ok(RuntimePathResolution::MissingPath) => 3,
            Ok(RuntimePathResolution::RegularFile | RuntimePathResolution::Directory) => 5,
            Err(_) => return Err(5),
        };
        if source_missing == 5 {
            return Err(5);
        }
        let normalized = resolve_path(requested_path, current_directory).map_err(|_| 123_u32)?;
        let snapshot = self.snapshot.as_ref().ok_or(3_u32)?;
        let relative =
            relative_to_mount(&normalized.identity, &snapshot.root_identity).map_err(|_| 3_u32)?;
        if snapshot.path_index.contains_key(relative) || self.registered_directory_exists(relative)
        {
            return Err(5); // The selected source is immutable.
        }
        if self.open_files.len() >= RUNTIME_SOURCE_FILE_LIMIT {
            return Err(4);
        }
        for open in self.open_files.values() {
            if resolve_path(&open.normalized_path_a, current_directory)
                .is_ok_and(|path| path.identity == normalized.identity)
                && file_access_conflicts(
                    desired_access,
                    share_mode,
                    open.desired_access,
                    open.share_mode,
                )
            {
                return Err(32);
            }
        }
        let id = RuntimeSourceFileId(self.next_file_id);
        let next_file_id = self.next_file_id.checked_add(1).ok_or(4_u32)?;
        Ok((
            RuntimeSourceFileOpen {
                next_file_id,
                file: RuntimeSourceFile {
                    id,
                    backing: RuntimeFileBacking::Opfs { byte_length: 0 },
                    normalized_path_a: normalized.bytes_with_nul,
                    cursor: 0,
                    desired_access,
                    share_mode,
                    flags_and_attributes,
                },
            },
            relative_path,
            source_missing,
            source_parents,
        ))
    }

    pub(super) fn preflight_opfs_open(
        &self,
        prepared: &RuntimeSourceFileOpen,
        current_directory: &[u8],
    ) -> Result<(), RuntimeStateError> {
        let (current, _, _, _) = self
            .prepare_opfs_open(
                &prepared.file.normalized_path_a,
                current_directory,
                prepared.file.desired_access,
                prepared.file.share_mode,
                prepared.file.flags_and_attributes,
            )
            .map_err(|_| {
                RuntimeStateError::new(
                    "file path, sharing, or capacity changed while awaiting OPFS",
                )
            })?;
        if &current != prepared {
            return Err(RuntimeStateError::new(
                "file identity changed while awaiting OPFS",
            ));
        }
        Ok(())
    }

    // No Rust state can change between prepare_open and this publication: the
    // aggregate registers the object/handle first, then calls this infallible
    // mutation without leaving Rust or invoking a guest callback.
    pub(super) fn open_validated(
        &mut self,
        prepared: RuntimeSourceFileOpen,
    ) -> RuntimeSourceFileId {
        self.next_file_id = prepared.next_file_id;
        let id = prepared.file.id;
        let previous = self.open_files.insert(id, prepared.file);
        debug_assert!(previous.is_none());
        id
    }

    pub(super) fn validate_retirement(
        &self,
        id: RuntimeSourceFileId,
    ) -> Result<(), RuntimeStateError> {
        self.open_file(id)?;
        if self
            .pending_reads
            .values()
            .any(|pending| pending.request.file_id == id)
        {
            return Err(RuntimeStateError::new(
                "logical source file cannot close while a read is pending",
            ));
        }
        Ok(())
    }

    // The aggregate has checked the file, pending I/O, and object retirement.
    // No host work or guest callback intervenes before this removal.
    pub(super) fn retire(&mut self, id: RuntimeSourceFileId) {
        debug_assert!(self.validate_retirement(id).is_ok());
        self.open_files
            .remove(&id)
            .expect("validated file retirement");
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
                    RuntimeStateError::new("open source file backing is missing from its snapshot")
                }),
            RuntimeFileBacking::Opfs { byte_length } => Ok(byte_length),
        }
    }

    pub(super) fn finish_opfs_write(
        &mut self,
        id: RuntimeSourceFileId,
        cursor: u64,
        transferred: u32,
    ) {
        let file = self.open_files.get_mut(&id).expect("preflighted OPFS file");
        let RuntimeFileBacking::Opfs { byte_length } = file.backing else {
            unreachable!("preflighted OPFS backing");
        };
        debug_assert_eq!(file.cursor, cursor);
        let end = cursor + u64::from(transferred);
        file.cursor = end;
        file.backing = RuntimeFileBacking::Opfs {
            byte_length: byte_length.max(end),
        };
    }

    pub(super) fn file_read_service(
        &self,
        id: RuntimeSourceFileId,
    ) -> Result<&'static str, RuntimeStateError> {
        match self.open_file(id)?.backing {
            RuntimeFileBacking::ImmutableSource { .. } => Ok(RUNTIME_SOURCE_READ_SERVICE),
            RuntimeFileBacking::Opfs { .. } => Ok(RUNTIME_OPFS_READ_SERVICE),
        }
    }

    pub(super) fn begin_read(
        &mut self,
        id: RuntimeSourceFileId,
        request_id: u64,
        buffer: GuestAddress,
        bytes_read: GuestAddress,
        requested_bytes: u32,
    ) -> Result<RuntimeSourceReadRequest, RuntimeStateError> {
        let file = self.open_file(id)?;
        let (source_id, object_id) = match file.backing {
            RuntimeFileBacking::ImmutableSource {
                source_id,
                object_id,
            } => (source_id, object_id),
            RuntimeFileBacking::Opfs { .. } => (
                0,
                u32::try_from(file.id.0).map_err(|_| {
                    RuntimeStateError::new("OPFS file ID exceeds the host transport")
                })?,
            ),
        };
        if file.desired_access & 0x8000_0000 == 0 {
            return Err(RuntimeStateError::new(
                "file was not opened with GENERIC_READ access",
            ));
        }
        if requested_bytes == 0 || requested_bytes > RUNTIME_SOURCE_READ_MAX_BYTES {
            return Err(RuntimeStateError::new(
                "source reads require a nonzero bounded byte count",
            ));
        }
        if self.pending_reads.contains_key(&request_id)
            || self
                .pending_reads
                .values()
                .any(|pending| pending.request.file_id == id)
        {
            return Err(RuntimeStateError::new(
                "logical source file already owns a pending read",
            ));
        }
        let pending = RuntimePendingSourceRead {
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
        };
        let request = pending.request;
        self.pending_reads.insert(request_id, pending);
        Ok(request)
    }

    pub(super) fn opfs_read_path(
        &self,
        request_id: u64,
        current_directory: &[u8],
    ) -> Result<String, RuntimeStateError> {
        let pending = self
            .pending_reads
            .get(&request_id)
            .ok_or_else(|| RuntimeStateError::new("OPFS read request has no pending file"))?;
        let file = self.open_file(pending.request.file_id)?;
        if !matches!(file.backing, RuntimeFileBacking::Opfs { .. }) {
            return Err(RuntimeStateError::new("read request is not an OPFS file"));
        }
        self.opfs_relative_path(&file.normalized_path_a, current_directory)
            .map_err(|_| RuntimeStateError::new("open file left the selected game directory"))
    }

    pub(super) fn plan_read_completion(
        &self,
        response: &RuntimeSourceReadHostResponse,
    ) -> Result<RuntimeSourceReadCompletionPlan, RuntimeStateError> {
        let pending = self
            .pending_reads
            .get(&response.request_id)
            .ok_or_else(|| {
                RuntimeStateError::new("source read completion does not identify a pending request")
            })?;
        let request = pending.request;
        let file = self.open_file(request.file_id)?;
        let byte_length = self.file_byte_length(request.file_id)?;
        if response.source_id != request.source_id
            || response.object_id != request.object_id
            || response.offset != request.offset
            || response.requested_bytes != request.requested_bytes
            || file.cursor != request.offset
        {
            return Err(RuntimeStateError::new(
                "source read completion identity or cursor does not match the pending request",
            ));
        }
        let succeeded = response.win32_error == 0;
        if succeeded {
            if response.transferred_bytes as usize != response.bytes.len()
                || response.transferred_bytes > response.requested_bytes
                || (response.transferred_bytes != 0
                    && response
                        .offset
                        .checked_add(u64::from(response.transferred_bytes))
                        .is_none_or(|end| end > byte_length))
            {
                return Err(RuntimeStateError::new(
                    "successful source read completion has invalid byte bounds",
                ));
            }
        } else if response.transferred_bytes != 0 || !response.bytes.is_empty() {
            return Err(RuntimeStateError::new(
                "failed source read completion may not carry bytes",
            ));
        }
        let next_cursor = file.cursor
            + if succeeded {
                u64::from(response.transferred_bytes)
            } else {
                0
            };
        let service = match file.backing {
            RuntimeFileBacking::ImmutableSource { .. } => RUNTIME_SOURCE_READ_SERVICE,
            RuntimeFileBacking::Opfs { .. } => RUNTIME_OPFS_READ_SERVICE,
        };
        Ok(RuntimeSourceReadCompletionPlan {
            request_id: request.request_id,
            file_id: request.file_id,
            buffer: pending.buffer,
            bytes_read: pending.bytes_read,
            next_cursor,
            service,
        })
    }

    pub(super) fn commit_read_completion(&mut self, plan: RuntimeSourceReadCompletionPlan) {
        self.pending_reads.remove(&plan.request_id);
        self.open_files
            .get_mut(&plan.file_id)
            .expect("validated read file remains live")
            .cursor = plan.next_cursor;
    }

    pub(super) fn pending_read_ids(&self, request_ids: &[u64]) -> Vec<u64> {
        request_ids
            .iter()
            .copied()
            .filter(|id| self.pending_reads.contains_key(id))
            .collect()
    }

    pub(super) fn cancel_reads(&mut self, request_ids: &[u64]) {
        assert!(
            request_ids
                .iter()
                .all(|id| self.pending_reads.contains_key(id)),
            "validated pending reads changed during thread exit"
        );
        for id in request_ids {
            self.pending_reads.remove(id);
        }
    }

    pub(super) fn seek_with(
        &mut self,
        id: RuntimeSourceFileId,
        distance: i64,
        origin: u32,
        max_cursor: u64,
        write_result: impl FnOnce(u64) -> Result<(), RuntimeStateError>,
    ) -> Result<Result<u64, u32>, RuntimeStateError> {
        let file = self.open_file(id)?;
        self.require_no_pending_file_operation(id)?;
        let byte_length = self.file_byte_length(id)?;
        let base = match origin {
            0 => 0i128,
            1 => i128::from(file.cursor),
            2 => i128::from(byte_length),
            _ => {
                return Ok(Err(ERROR_INVALID_PARAMETER));
            }
        };
        let cursor = base + i128::from(distance);
        // Seeking beyond EOF does not extend the file. The caller's ABI bounds
        // the result; the owned cursor and source read transport are 64-bit.
        if cursor < 0 {
            return Ok(Err(ERROR_NEGATIVE_SEEK));
        }
        if cursor > i128::from(max_cursor) {
            return Ok(Err(ERROR_INVALID_PARAMETER));
        }
        let cursor = cursor as u64;
        write_result(cursor)?;
        // Checked guest output has committed. Cursor publication cannot fail.
        self.open_files
            .get_mut(&id)
            .expect("validated open file")
            .cursor = cursor;
        Ok(Ok(cursor))
    }

    fn require_no_pending_file_operation(
        &self,
        id: RuntimeSourceFileId,
    ) -> Result<(), RuntimeStateError> {
        if self
            .pending_reads
            .values()
            .any(|pending| pending.request.file_id == id)
        {
            return Err(RuntimeStateError::new(
                "logical file already owns a pending read",
            ));
        }
        Ok(())
    }
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
    let body = nul_terminated_body(requested)?;
    let separators = windows_932_separator_offsets(body)?;
    let (directory, file_name_pattern) = if let Some(separator) = separators.last().copied() {
        let directory_end = if separator == 2 && is_absolute(body) {
            separator + 1
        } else {
            separator
        };
        if directory_end == 0 || separator + 1 == body.len() {
            return Err(RuntimeStateError::new(
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
        return Err(RuntimeStateError::new(
            "file search final component must be nonempty and at most 260 bytes",
        ));
    }
    let mut index = 0;
    while index < component.len() {
        let byte = component[index];
        if is_windows_932_lead_byte(byte) {
            let Some(&trail) = component.get(index + 1) else {
                return Err(RuntimeStateError::new(
                    "file search pattern ends in a Windows-932 lead byte",
                ));
            };
            if !is_windows_932_trail_byte(trail) {
                return Err(RuntimeStateError::new(
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
            return Err(RuntimeStateError::new(
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
                return Err(RuntimeStateError::new(
                    "file search path ends in a Windows-932 lead byte",
                ));
            };
            if !is_windows_932_trail_byte(trail) {
                return Err(RuntimeStateError::new(
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
                return Err(RuntimeStateError::new(
                    "file search text ends in a Windows-932 lead byte",
                ));
            };
            if !is_windows_932_trail_byte(trail) {
                return Err(RuntimeStateError::new(
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
    let body = nul_terminated_body(requested)?;
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
    let body = nul_terminated_body(bytes)?;
    if !is_absolute(body) {
        return Err(RuntimeStateError::new(
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
        return Err(RuntimeStateError::new(
            "source manifest relative path is empty, absolute, NUL-containing, or too long",
        ));
    }
    let components = normalize_components(bytes, false)?;
    if components.is_empty() {
        return Err(RuntimeStateError::new(
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
                        return Err(RuntimeStateError::new(
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
                return Err(RuntimeStateError::new(
                    "source path ends in a Windows-932 lead byte",
                ));
            };
            if !is_windows_932_trail_byte(trail) {
                return Err(RuntimeStateError::new(
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
                return Err(RuntimeStateError::new(
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
        return Err(RuntimeStateError::new(
            "source path trailing dot/space normalization is outside the first VFS slice",
        ));
    }
    Ok(())
}

fn relative_to_mount<'a>(path: &'a [u8], root: &[u8]) -> Result<&'a [u8], RuntimeStateError> {
    if path == root {
        return Err(RuntimeStateError::new(
            "source file open path identifies the mount root directory",
        ));
    }
    if path.len() <= root.len() || !path.starts_with(root) || path.get(root.len()) != Some(&b'\\') {
        return Err(RuntimeStateError::new(
            "source file path is outside the registered immutable mount",
        ));
    }
    Ok(&path[root.len() + 1..])
}

fn nul_terminated_body(bytes: &[u8]) -> Result<&[u8], RuntimeStateError> {
    if bytes.len() < 2
        || bytes.len() > 32 * 1024
        || bytes.last() != Some(&0)
        || bytes[..bytes.len() - 1].contains(&0)
    {
        return Err(RuntimeStateError::new(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_source_discovery_matches_only_valid_windows_932_names_under_mount() {
        let mut files = RuntimeFiles::default();
        files.register_snapshot(manifest()).unwrap();
        assert_eq!(
            files
                .source_search_directory(b"Data\\*.dat\0", b"C:\\Vine\0")
                .unwrap(),
            Some(b"Data".to_vec())
        );
        assert_eq!(
            files
                .source_search_directory(b"*.dat\0", b"C:\\Vine\0")
                .unwrap(),
            Some(Vec::new())
        );
        assert_eq!(
            files
                .source_search_directory(b"Data\\entry.dat\0", b"C:\\Vine\0")
                .unwrap(),
            None
        );
        assert!(
            files
                .source_search_directory(b"D:\\outside\\*.dat\0", b"C:\\Vine\0")
                .is_err()
        );
        let names = vec![
            ("other.bin".to_owned(), true),
            ("sample.DAT".to_owned(), true),
            ("日本語.dat".to_owned(), true),
            ("Sub.dat".to_owned(), false),
            ("😀.dat".to_owned(), true),
        ];
        let (actual, matches) = files
            .source_search_matches(
                b"data\\*.dat\0",
                b"C:\\Vine\0",
                &["Data".to_owned()],
                &names,
            )
            .unwrap();
        assert_eq!(actual, b"Data");
        assert_eq!(matches.len(), 3);
        assert!(matches.contains(&(b"Data\\sample.DAT".to_vec(), true)));
        assert!(matches.contains(&(b"Data\\Sub.dat".to_vec(), false)));
        let (encoded, _, _) = SHIFT_JIS.encode("日本語.dat");
        assert!(matches.contains(&([b"Data\\".as_slice(), encoded.as_ref()].concat(), true)));
        assert!(
            files
                .source_search_matches(
                    b"Data\\*.dat\0",
                    b"C:\\Vine\0",
                    &["Data".to_owned()],
                    &[
                        ("sample.DAT".to_owned(), true),
                        ("SAMPLE.dat".to_owned(), true)
                    ]
                )
                .is_err()
        );

        assert!(
            files
                .source_search_matches(
                    b"Data\\*.dat\0",
                    b"C:\\Vine\0",
                    &["Other".to_owned()],
                    &names
                )
                .is_err()
        );
    }

    #[test]
    fn find_data_encoding_preserves_native_fields_and_filename_bounds() {
        let mut entry = RuntimeFileSearchEntry {
            file_name_a: vec![b'a'; 259],
            kind: RuntimeFileSearchEntryKind::RegularFile,
            byte_length: 0x1234_5678_9abc_def0,
        };
        let bytes = entry.encode_find_data_a().unwrap();
        assert_eq!(bytes.len(), 320);
        assert_eq!(&bytes[..4], &0x20_u32.to_le_bytes());
        assert_eq!(&bytes[4..28], &[0; 24]);
        assert_eq!(&bytes[28..32], &0x1234_5678_u32.to_le_bytes());
        assert_eq!(&bytes[32..36], &0x9abc_def0_u32.to_le_bytes());
        assert_eq!(&bytes[36..44], &[0; 8]);
        assert_eq!(&bytes[44..303], &[b'a'; 259]);
        assert_eq!(&bytes[303..], &[0; 17]);
        entry.kind = RuntimeFileSearchEntryKind::Directory;
        assert_eq!(
            &entry.encode_find_data_a().unwrap()[..4],
            &0x30_u32.to_le_bytes()
        );
        for name in [Vec::new(), vec![b'a'; 260], b"bad\0name".to_vec()] {
            entry.file_name_a = name;
            assert!(entry.encode_find_data_a().is_err());
        }
    }

    #[test]
    fn source_search_batch_rejects_late_errors_without_publishing_or_consuming_ids() {
        for invalid in [
            (b"New\\bad.dat".to_vec(), Some((1, "bad digest".to_owned()))),
            (
                b"New\\bad.dat".to_vec(),
                Some((u64::from(u32::MAX) + 1, "a".repeat(64))),
            ),
            (b"NEW\\FIRST.DAT".to_vec(), Some((1, "a".repeat(64)))),
            (b"NEW\\FIRST.DAT".to_vec(), None),
            (b"..\\escape".to_vec(), None),
        ] {
            let mut files = RuntimeFiles::default();
            files.register_snapshot(manifest()).unwrap();
            let directories_before = files.snapshot().unwrap().directories.len();
            assert!(
                files
                    .register_source_search_batch(
                        b"New",
                        vec![
                            (b"New\\first.dat".to_vec(), Some((4, "A".repeat(64)))),
                            invalid,
                        ]
                    )
                    .is_err()
            );
            let snapshot = files.snapshot().unwrap();
            assert_eq!(snapshot.files.len(), 1);
            assert_eq!(snapshot.path_index.len(), 1);
            assert_eq!(snapshot.directories.len(), directories_before);
            assert!(!snapshot.directories.contains_key(b"new".as_slice()));
            assert_eq!(
                files
                    .register_source_search_batch(
                        b"New",
                        vec![(b"New\\first.dat".to_vec(), Some((4, "A".repeat(64)))),]
                    )
                    .unwrap(),
                vec![(0, 12)]
            );
            let snapshot = files.snapshot().unwrap();

            assert_eq!(snapshot.path_index[b"new\\first.dat".as_slice()], 12);
            assert_eq!(snapshot.directories[b"new".as_slice()], b"New");
        }
        let mut files = RuntimeFiles::default();
        assert!(files.register_source_search_batch(b"", Vec::new()).is_err());
        files.register_snapshot(manifest()).unwrap();
        assert!(
            files
                .register_source_search_batch(b"", Vec::new())
                .unwrap()
                .is_empty()
        );
        assert!(
            files
                .register_source_search_batch(
                    b"New",
                    vec![
                        (b"New\\folder".to_vec(), None),
                        (b"NEW\\FOLDER".to_vec(), Some((1, "a".repeat(64)))),
                    ]
                )
                .is_err()
        );
        assert!(
            !files
                .snapshot()
                .unwrap()
                .directories
                .contains_key(b"new".as_slice())
        );
    }

    #[test]
    fn source_search_batch_checks_id_and_capacity_before_publication() {
        let mut files = RuntimeFiles::default();
        let mut source = manifest();
        source.files[0].object_id = u32::MAX;
        files.register_snapshot(source).unwrap();
        assert!(
            files
                .register_source_search_batch(
                    b"New",
                    vec![(b"New\\one.dat".to_vec(), Some((0, "a".repeat(64)))),]
                )
                .is_err()
        );
        assert_eq!(files.snapshot().unwrap().files.len(), 1);
        assert!(
            !files
                .snapshot()
                .unwrap()
                .directories
                .contains_key(b"new".as_slice())
        );
        assert!(
            files
                .register_source_search_batch(b"New", Vec::new())
                .unwrap()
                .is_empty()
        );

        let mut files = RuntimeFiles::default();
        let mut source = manifest();
        source.files = (1..RUNTIME_SOURCE_FILE_LIMIT)
            .map(|index| RuntimeSourceFileManifest {
                object_id: u32::try_from(index).unwrap(),
                relative_path_a: format!("Data\\{index}.dat").into_bytes(),
                byte_length: 0,
                sha256: "a".repeat(64),
            })
            .collect();
        files.register_snapshot(source).unwrap();
        assert!(
            files
                .register_source_search_batch(
                    b"New",
                    vec![
                        (b"New\\one.dat".to_vec(), Some((0, "a".repeat(64)))),
                        (b"New\\two.dat".to_vec(), Some((0, "a".repeat(64)))),
                    ]
                )
                .is_err()
        );
        assert_eq!(
            files.snapshot().unwrap().files.len(),
            RUNTIME_SOURCE_FILE_LIMIT - 1
        );
        assert!(
            !files
                .snapshot()
                .unwrap()
                .directories
                .contains_key(b"new".as_slice())
        );
        assert_eq!(
            files
                .register_source_search_batch(
                    b"New",
                    vec![(b"New\\one.dat".to_vec(), Some((0, "a".repeat(64)))),]
                )
                .unwrap(),
            vec![(0, u32::try_from(RUNTIME_SOURCE_FILE_LIMIT).unwrap())]
        );
    }

    #[test]
    fn registered_empty_source_directory_is_enumerable_without_a_file_catalog_entry() {
        let mut files = RuntimeFiles::default();
        files.register_snapshot(manifest()).unwrap();
        files.register_source_directory(b"EmPtY").unwrap();
        let search = files
            .new_file_search(b"EMPTY\0", b"C:\\Vine\0")
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(search.entries.len(), 1);
        assert_eq!(search.entries[0].file_name_a, b"EmPtY");
        assert_eq!(
            search.entries[0].kind,
            RuntimeFileSearchEntryKind::Directory
        );
        assert_eq!(search.entries[0].byte_length, 0);
        assert!(
            files
                .register_source_file(RuntimeSourceFileManifest {
                    object_id: 12,
                    relative_path_a: b"empty".to_vec(),
                    byte_length: 0,
                    sha256: "0".repeat(64),
                })
                .is_err()
        );
        let snapshot = files.snapshot().unwrap();
        assert!(snapshot.directories.contains_key(b"empty".as_slice()));
        assert!(!snapshot.files.contains_key(&12));
        assert!(files.searches.is_empty());
        let retry = files
            .new_file_search(b"EMPTY\0", b"C:\\Vine\0")
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(retry.entries[0].file_name_a, b"EmPtY");
    }

    #[test]
    fn searches_reject_missing_paths_and_unknown_cursors_without_disturbing_live_enumeration() {
        let mut files = RuntimeFiles::default();
        files.register_snapshot(manifest()).unwrap();
        let search = files
            .new_file_search(b"Data\\*\0", b"C:\\Vine\0")
            .unwrap()
            .unwrap()
            .unwrap();
        let id = search.id;
        let first = search.entries[0].clone();
        files.insert_file_search(search);
        assert!(files.next_file_search_entry(id).unwrap().is_none());
        assert!(matches!(
            files.new_file_search(b"Data\\nothing.dat\0", b"C:\\Vine\0"),
            Ok(Ok(None))
        ));
        for missing in [b"missing\\*\0".as_slice(), b"D:\\outside\\*\0"] {
            assert!(matches!(
                files.new_file_search(missing, b"C:\\Vine\0"),
                Ok(Err(3))
            ));
        }
        assert!(
            files
                .next_file_search_entry(RuntimeFileSearchId(999))
                .is_err()
        );
        assert!(files.next_file_search_entry(id).unwrap().is_none());
        let retry = files
            .new_file_search(b"Data\\*\0", b"C:\\Vine\0")
            .unwrap()
            .unwrap()
            .unwrap();
        assert_ne!(retry.id, id);
        assert_eq!(retry.entries[0], first);
        let retry_id = retry.id;
        files.insert_file_search(retry);
        assert!(files.next_file_search_entry(id).unwrap().is_none());
        assert!(files.next_file_search_entry(retry_id).unwrap().is_none());
    }

    fn source_read_response_control(
        requested_bytes: u32,
        transferred_bytes: u32,
        win32_error: u32,
    ) -> [u8; RUNTIME_SOURCE_READ_RESPONSE_CONTROL_BYTES] {
        let words = [7, 11, 3, 0, requested_bytes, transferred_bytes, win32_error];
        let mut control = [0; RUNTIME_SOURCE_READ_RESPONSE_CONTROL_BYTES];
        for (word, bytes) in words.into_iter().zip(control.chunks_exact_mut(4)) {
            bytes.copy_from_slice(&word.to_le_bytes());
        }
        control
    }

    fn manifest() -> RuntimeSourceSnapshotManifest {
        RuntimeSourceSnapshotManifest {
            mount: RuntimeSourceMountManifest {
                source_id: 7,
                root_path_a: b"C:\\Vine\0".to_vec(),
            },
            files: vec![RuntimeSourceFileManifest {
                object_id: 11,
                relative_path_a: b"Data\\ARC.gdb".to_vec(),
                byte_length: 9,
                sha256: "0".repeat(64),
            }],
        }
    }

    #[test]
    fn later_source_file_registration_is_sandboxed_and_failure_atomic() {
        let mut files = RuntimeFiles::default();
        files.register_snapshot(manifest()).unwrap();

        assert_eq!(
            files
                .source_lookup_relative_path(b"C:\\Vine\\Data\\archive.arc\0", b"C:\\Vine\0")
                .unwrap(),
            Some(b"Data\\archive.arc".to_vec())
        );
        assert!(
            files
                .source_lookup_relative_path(b"D:\\other.arc\0", b"C:\\Vine\0")
                .is_err()
        );
        let file = RuntimeSourceFileManifest {
            object_id: 12,
            relative_path_a: b"Data\\archive.arc".to_vec(),
            byte_length: 7,
            sha256: "a".repeat(64),
        };
        for invalid in [
            RuntimeSourceFileManifest {
                relative_path_a: b"..\\outside".to_vec(),
                ..file.clone()
            },
            RuntimeSourceFileManifest {
                relative_path_a: b"data\\ARC.gdb".to_vec(),
                ..file.clone()
            },
            RuntimeSourceFileManifest {
                sha256: "invalid".to_owned(),
                ..file.clone()
            },
        ] {
            assert!(files.register_source_file(invalid).is_err());
            let snapshot = files.snapshot().unwrap();
            assert_eq!(snapshot.source_id, 7);
            assert_eq!(snapshot.files.len(), 1);
            assert_eq!(snapshot.path_index.len(), 1);
            assert_eq!(snapshot.files[&11].byte_length, 9);
            assert!(!snapshot.files.contains_key(&12));
            assert!(files.open_files.is_empty());
        }
        files.register_source_file(file.clone()).unwrap();
        assert_eq!(
            files
                .source_lookup_relative_path(b"C:\\Vine\\data\\ARCHIVE.arc\0", b"C:\\Vine\0")
                .unwrap(),
            None
        );
        assert_eq!(
            files
                .resolve_path_kind(b"C:\\Vine\\Data\\ARCHIVE.arc\0", b"C:\\Vine\0")
                .unwrap(),
            RuntimePathResolution::RegularFile
        );
        assert!(files.register_source_file(file).is_err());
        let snapshot = files.snapshot().unwrap();
        assert_eq!(snapshot.files.len(), 2);
        assert_eq!(snapshot.path_index.len(), 2);
        assert_eq!(snapshot.files[&11].byte_length, 9);
        assert_eq!(snapshot.files[&12].byte_length, 7);

        assert!(files.open_files.is_empty());
        assert_eq!(files.next_file_id, 1);
    }

    #[test]
    fn opfs_paths_preserve_spelling_and_windows_932_without_leaving_the_mount() {
        let mut files = RuntimeFiles::default();
        files.register_snapshot(manifest()).unwrap();
        for (path, expected) in [
            (b"Save\\Slot01\0".as_slice(), "Save/Slot01"),
            (b"c:\\vInE\\Save\\Slot01\0".as_slice(), "Save/Slot01"),
            (b"Save\\\x83\x5c\0".as_slice(), "Save/ソ"),
            (b"Data\\\x83\x5c\\slot.dat\0".as_slice(), "Data/ソ/slot.dat"),
        ] {
            assert_eq!(
                files.opfs_relative_path(path, b"C:\\Vine\0").unwrap(),
                expected
            );
        }
        for (path, error) in [
            (b"..\\Outside\0".as_slice(), 3),
            (b"D:\\Save\0".as_slice(), 3),
            (b"\\\\server\\share\0".as_slice(), 3),
            (b"\\Save\0".as_slice(), 3),
            (b"C:\\Vine\0".as_slice(), 183),
            (b"Save\\..\\..\\Outside\0".as_slice(), 3),
            (b"Save\0extra\0".as_slice(), 123),
            (b"Save\\\x83\0".as_slice(), 123),
            (b"Data\\ARC.gdb\\slot.dat\0".as_slice(), 3),
        ] {
            assert_eq!(files.opfs_relative_path(path, b"C:\\Vine\0"), Err(error));
        }
    }

    #[test]
    fn opfs_directory_create_only_mirrors_verified_source_parents() {
        let mut files = RuntimeFiles::default();
        files.register_snapshot(manifest()).unwrap();
        for (path, expected) in [
            (b"NewRoot\0".as_slice(), Ok(("NewRoot".to_owned(), 0))),
            (b"Data\\Slot\0".as_slice(), Ok(("Data/Slot".to_owned(), 1))),
            (
                b"Data\\Sub\\Slot\0".as_slice(),
                Ok(("Data/Sub/Slot".to_owned(), 1)),
            ),
            (
                b"Data\\\x83\x5c\\Slot\0".as_slice(),
                Ok(("Data/ソ/Slot".to_owned(), 1)),
            ),
            (b"Data\\ARC.gdb\\Slot\0".as_slice(), Err(3)),
            (b"Data\\ARC.gdb\0".as_slice(), Err(183)),
            (b"D:\\Other\\Slot\0".as_slice(), Err(3)),
        ] {
            assert_eq!(
                files.opfs_create_directory_path(path, b"C:\\Vine\0"),
                expected
            );
        }
    }

    #[test]
    fn windows_932_trailing_separator_byte_is_not_a_path_boundary() {
        let mut files = RuntimeFiles::default();
        files.register_snapshot(manifest()).unwrap();
        assert_eq!(
            files
                .resolve_path_kind(b"Data\\\x83\x5c\0", b"C:\\Vine\0")
                .unwrap(),
            RuntimePathResolution::MissingFile
        );
        assert_eq!(
            files
                .resolve_path_kind(b"Data\\\x83\x5c\\slot.dat\0", b"C:\\Vine\0")
                .unwrap(),
            RuntimePathResolution::MissingPath
        );
        files.register_source_directory(b"Data\\\x83\x5c").unwrap();
        assert_eq!(
            files.opfs_create_directory_path(b"Data\\\x83\x5c\\Slot\0", b"C:\\Vine\0"),
            Ok(("Data/ソ/Slot".to_owned(), 2))
        );
    }

    #[test]
    fn opfs_file_open_preflights_source_sharing_and_handle_before_publication() {
        let mut files = RuntimeFiles::default();
        files.register_snapshot(manifest()).unwrap();
        assert_eq!(
            files
                .prepare_opfs_open(b"Data\\ARC.gdb\0", b"C:\\Vine\0", 0x4000_0000, 0, 0x20)
                .unwrap_err(),
            5
        );
        assert_eq!(
            files
                .prepare_opfs_open(
                    b"D:\\Outside\\save.dat\0",
                    b"C:\\Vine\0",
                    0x4000_0000,
                    0,
                    0x20
                )
                .unwrap_err(),
            3
        );
        let (prepared, relative, missing, source_parents) = files
            .prepare_opfs_open(b"Data\\save.dat\0", b"C:\\Vine\0", 0x4000_0000, 0, 0x20)
            .unwrap();
        assert_eq!(
            (relative.as_str(), missing, source_parents),
            ("Data/save.dat", 2, 1)
        );
        assert!(files.open_files.is_empty());
        assert!(files.pending_reads.is_empty());
        assert_eq!(files.next_file_id, 1);
        assert_eq!(prepared.file_id().0, 1);
        files.preflight_opfs_open(&prepared, b"C:\\Vine\0").unwrap();
        let open = files.open_validated(prepared.confirmed_opfs(0));
        assert_eq!(files.file_byte_length(open).unwrap(), 0);
        assert_eq!(
            files
                .prepare_opfs_open(b"data\\SAVE.dat\0", b"C:\\Vine\0", 0x8000_0000, 1, 0x20)
                .unwrap_err(),
            32
        );
        assert_eq!(
            files
                .seek_with(open, 3, 0, i64::MAX as u64, |_| Ok(()))
                .unwrap(),
            Ok(3)
        );
        files.validate_retirement(open).unwrap();
        files.retire(open);
        assert!(files.open_file(open).is_err());
        let (reopened, _, _, _) = files
            .prepare_opfs_open(b"Data\\save.dat\0", b"C:\\Vine\0", 0x8000_0000, 1, 0x20)
            .unwrap();
        assert_eq!(reopened.file_id().0, 2);
        files.preflight_opfs_open(&reopened, b"C:\\Vine\0").unwrap();
        let reopened = files.open_validated(reopened.confirmed_opfs(5));
        assert_eq!(files.open_file(reopened).unwrap().cursor, 0);
        assert_eq!(files.file_byte_length(reopened).unwrap(), 5);
    }

    #[test]
    fn opfs_delete_rejects_source_directories_and_live_handles_without_mutation() {
        let mut files = RuntimeFiles::default();
        files.register_snapshot(manifest()).unwrap();
        assert_eq!(
            files.opfs_delete_path(b"Data\\ARC.gdb\0", b"C:\\Vine\0"),
            Err(5)
        );
        assert_eq!(files.opfs_delete_path(b"Data\0", b"C:\\Vine\0"), Err(5));
        assert_eq!(
            files.opfs_delete_path(b"D:\\outside.dat\0", b"C:\\Vine\0"),
            Err(3)
        );
        assert_eq!(
            files.opfs_delete_path(b"Save.dat\0", b"C:\\Vine\0"),
            Ok(("Save.dat".to_owned(), 2))
        );
        assert_eq!(
            files.opfs_delete_path(b"Data\\missing.dat\0", b"C:\\Vine\0"),
            Ok(("Data/missing.dat".to_owned(), 2))
        );
        assert_eq!(
            files.opfs_delete_path(b"Data\\\x83\x5c\0", b"C:\\Vine\0"),
            Ok(("Data/ソ".to_owned(), 2))
        );
        assert_eq!(
            files.opfs_delete_path(b"Data\\ARC.gdb\\slot.dat\0", b"C:\\Vine\0"),
            Err(3)
        );
        assert_eq!(
            files.opfs_delete_path(b"unknown\\missing.dat\0", b"C:\\Vine\0"),
            Ok(("unknown/missing.dat".to_owned(), 3))
        );
        files.open_files.insert(
            RuntimeSourceFileId(42),
            RuntimeSourceFile {
                id: RuntimeSourceFileId(42),
                backing: RuntimeFileBacking::Opfs { byte_length: 0 },
                normalized_path_a: b"C:\\Vine\\save.DAT\0".to_vec(),
                cursor: 0,
                desired_access: 0x8000_0000,
                share_mode: 1,
                flags_and_attributes: 0x80,
            },
        );
        assert_eq!(
            files.opfs_delete_path(b"SAVE.dat\0", b"C:\\Vine\0"),
            Err(32)
        );
    }

    #[test]
    fn source_registration_accepts_uppercase_digest_and_retains_path_and_length() {
        let mut files = RuntimeFiles::default();
        let mut source = manifest();
        source.files[0].sha256 = "AB".repeat(32);
        files.register_snapshot(source).unwrap();

        files
            .register_source_file(RuntimeSourceFileManifest {
                object_id: 12,
                relative_path_a: b"second.dat".to_vec(),
                byte_length: 7,
                sha256: "CD".repeat(32),
            })
            .unwrap();
        let second = &files.snapshot().unwrap().files[&12];
        assert_eq!(second.relative_path_a, b"second.dat");
        assert_eq!(second.byte_length, 7);
    }

    #[test]
    fn registration_rejects_duplicate_source_paths_without_mutation() {
        let mut files = RuntimeFiles::default();
        let mut source = manifest();
        source.files.push(RuntimeSourceFileManifest {
            object_id: 12,
            relative_path_a: b"data\\arc.GDB".to_vec(),
            byte_length: 1,
            sha256: "0".repeat(64),
        });
        assert!(files.register_snapshot(source).is_err());
        assert!(files.snapshot().is_none());
        assert!(files.open_files.is_empty());
        assert!(files.pending_reads.is_empty());
        assert_eq!(files.next_file_id, 1);
        assert_eq!(files.next_search_id, 1);

        files.register_snapshot(manifest()).unwrap();
    }

    #[test]
    fn source_snapshot_paths_are_case_insensitive_and_mount_bounded() {
        let mut files = RuntimeFiles::default();
        files.register_snapshot(manifest()).unwrap();
        assert!(files.register_snapshot(manifest()).is_err());
        assert_eq!(files.snapshot().unwrap().source_id, 7);
        assert_eq!(files.snapshot().unwrap().files.len(), 1);
        assert!(files.open_files.is_empty());
        assert_eq!(files.next_file_id, 1);
        let SourceOpenOutcome::Opened(opened) = files
            .prepare_open_with_disposition(
                b"c:/vine/data/../DATA/arc.GDB\0",
                b"C:\\Vine\0",
                0x8000_0000,
                1,
                3,
                0x0800_0020,
            )
            .unwrap()
        else {
            panic!("known source path must open");
        };
        assert_eq!(
            opened.file.backing,
            RuntimeFileBacking::ImmutableSource {
                source_id: 7,
                object_id: 11,
            }
        );
        assert!(
            files
                .prepare_open_with_disposition(
                    b"D:\\Vine\\Data\\ARC.gdb\0",
                    b"C:\\Vine\0",
                    0x8000_0000,
                    1,
                    3,
                    0x0800_0020,
                )
                .is_err()
        );
    }

    #[test]
    fn source_open_rejects_creation_truncation_and_write_without_mutation() {
        let mut files = RuntimeFiles::default();
        files.register_snapshot(manifest()).unwrap();
        for access in [0x8000_0000, 0x4000_0000, 0xc000_0000] {
            for disposition in 1..=5 {
                if access == 0x8000_0000 && disposition == 3 {
                    continue;
                }
                assert!(
                    files
                        .prepare_open_with_disposition(
                            b"Data\\ARC.gdb\0",
                            b"C:\\Vine\0",
                            access,
                            0,
                            disposition,
                            0x20,
                        )
                        .is_err()
                );
            }
        }
        assert!(
            matches!(
                files
                    .prepare_open_with_disposition(
                        b"unlisted-save.dat\0",
                        b"C:\\Vine\0",
                        0xc000_0000,
                        0,
                        4,
                        0x20,
                    )
                    .unwrap(),
                SourceOpenOutcome::Missing
            ),
            "unknown writable paths must defer to OPFS"
        );
    }

    #[test]
    fn path_resolution_derives_registered_directories_without_mutation() {
        let mut files = RuntimeFiles::default();
        files.register_snapshot(manifest()).unwrap();

        for (path, expected) in [
            (b"C:\\Vine\0".as_slice(), RuntimePathResolution::Directory),
            (b"data\0".as_slice(), RuntimePathResolution::Directory),
            (
                b"DATA\\arc.GDB\0".as_slice(),
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
            (b"ARC.gdb\0".as_slice(), RuntimePathResolution::MissingFile),
            (
                b"D:\\Vine\\Data\\ARC.gdb\0".as_slice(),
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
    }

    #[test]
    fn pending_read_advances_cursor_only_after_matching_completion() {
        let mut files = RuntimeFiles::default();
        files.register_snapshot(manifest()).unwrap();
        let SourceOpenOutcome::Opened(open) = files
            .prepare_open_with_disposition(
                b"C:\\Vine\\Data\\ARC.gdb\0",
                b"C:\\Vine\0",
                0x8000_0000,
                1,
                3,
                0x0800_0020,
            )
            .unwrap()
        else {
            panic!("known source path must open");
        };
        let id = open.file_id();
        files.open_validated(open);
        files
            .begin_read(id, 3, GuestAddress(0x1000), GuestAddress(0x2000), 4)
            .unwrap();
        assert_eq!(files.open_file(id).unwrap().cursor, 0);

        assert!(files.validate_retirement(id).is_err());
        assert!(
            files
                .seek_with(id, 1, 0, i64::MAX as u64, |_| Ok(()))
                .is_err()
        );
        assert_eq!(files.open_file(id).unwrap().cursor, 0);
        assert_eq!(files.pending_reads.len(), 1);
        assert_eq!(files.pending_reads[&3].request.file_id, id);
        for request_id in [3, 4] {
            assert!(
                files
                    .begin_read(
                        id,
                        request_id,
                        GuestAddress(0x3000),
                        GuestAddress(0x4000),
                        2
                    )
                    .is_err()
            );
            assert_eq!(files.pending_reads.len(), 1);
            let pending = &files.pending_reads[&3];
            assert_eq!(pending.buffer, GuestAddress(0x1000));
            assert_eq!(pending.bytes_read, GuestAddress(0x2000));
            assert_eq!(pending.request.requested_bytes, 4);
            assert_eq!(files.open_file(id).unwrap().cursor, 0);
        }
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
        assert!(files.plan_read_completion(&wrong).is_err());
        assert_eq!(files.open_file(id).unwrap().cursor, 0);
        assert_eq!(files.pending_reads.len(), 1);
        assert_eq!(files.pending_reads[&3].request.object_id, 11);

        wrong.object_id = 11;
        let plan = files.plan_read_completion(&wrong).unwrap();
        assert_eq!(plan.buffer(), GuestAddress(0x1000));
        assert_eq!(plan.bytes_read(), GuestAddress(0x2000));
        files.commit_read_completion(plan);
        assert_eq!(files.open_file(id).unwrap().cursor, 4);
        assert!(files.pending_reads.is_empty());
        assert!(files.plan_read_completion(&wrong).is_err());
        assert_eq!(files.open_file(id).unwrap().cursor, 4);
        files.validate_retirement(id).unwrap();
    }

    #[test]
    fn seek_beyond_eof_returns_zero_on_read_without_moving_the_cursor() {
        let mut files = RuntimeFiles::default();
        files.register_snapshot(manifest()).unwrap();
        let SourceOpenOutcome::Opened(prepared) = files
            .prepare_open_with_disposition(
                b"Data\\ARC.gdb\0",
                b"C:\\Vine\0",
                0x8000_0000,
                1,
                3,
                0x20,
            )
            .unwrap()
        else {
            panic!("known source file must open");
        };
        let id = files.open_validated(prepared);
        assert_eq!(
            files
                .seek_with(id, 10, 0, i64::MAX as u64, |_| Ok(()))
                .unwrap(),
            Ok(10)
        );
        let request = files
            .begin_read(id, 40, GuestAddress(0x1000), GuestAddress(0x2000), 4)
            .unwrap();
        assert_eq!(request.offset, 10);
        let empty = RuntimeSourceReadHostResponse {
            request_id: 40,
            source_id: 7,
            object_id: 11,
            offset: 10,
            requested_bytes: 4,
            transferred_bytes: 0,
            win32_error: 0,
            bytes: Vec::new(),
        };
        let mut empty = empty;
        empty.transferred_bytes = 1;
        empty.bytes.push(0);
        assert!(files.plan_read_completion(&empty).is_err());
        assert_eq!(files.open_file(id).unwrap().cursor, 10);
        assert_eq!(files.pending_reads.len(), 1);
        assert_eq!(files.pending_reads[&40].request.file_id, id);
        empty.transferred_bytes = 0;
        empty.bytes.clear();
        files.commit_read_completion(files.plan_read_completion(&empty).unwrap());
        assert_eq!(files.open_file(id).unwrap().cursor, 10);
        assert_eq!(
            files
                .seek_with(id, -11, 1, i64::MAX as u64, |_| Ok(()))
                .unwrap(),
            Err(ERROR_NEGATIVE_SEEK)
        );
        assert_eq!(files.open_file(id).unwrap().cursor, 10);
        assert_eq!(
            files
                .seek_with(id, 0, 3, i64::MAX as u64, |_| Ok(()))
                .unwrap(),
            Err(ERROR_INVALID_PARAMETER)
        );
        assert_eq!(files.open_file(id).unwrap().cursor, 10);
        assert_eq!(
            files
                .seek_with(id, i64::MAX, 1, i64::MAX as u64, |_| Ok(()))
                .unwrap(),
            Err(ERROR_INVALID_PARAMETER)
        );
        assert_eq!(files.open_file(id).unwrap().cursor, 10);
        assert_eq!(
            files
                .seek_with(id, -1, 1, i64::MAX as u64, |_| Ok(()))
                .unwrap(),
            Ok(9)
        );
        assert_eq!(files.open_file(id).unwrap().cursor, 9);
        let before = files.open_file(id).unwrap().clone();
        assert!(
            files
                .seek_with(id, 1, 1, i64::MAX as u64, |_| {
                    Err(RuntimeStateError::new("checked output failed"))
                })
                .is_err()
        );
        assert_eq!(files.open_file(id).unwrap(), &before);
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

        let maximum_control = source_read_response_control(RUNTIME_SOURCE_READ_MAX_BYTES, 0, 5);
        let maximum =
            RuntimeSourceReadHostResponse::decode_binary(11, &maximum_control, &[]).unwrap();
        assert_eq!(maximum.requested_bytes, RUNTIME_SOURCE_READ_MAX_BYTES);

        for length in [
            0,
            RUNTIME_SOURCE_READ_RESPONSE_CONTROL_BYTES - 1,
            RUNTIME_SOURCE_READ_RESPONSE_CONTROL_BYTES + 1,
        ] {
            assert!(
                RuntimeSourceReadHostResponse::decode_binary(12, &vec![0; length], &[]).is_err()
            );
        }

        for (control, payload) in [
            (source_read_response_control(0, 0, 0), Vec::new()),
            (
                source_read_response_control(RUNTIME_SOURCE_READ_MAX_BYTES + 1, 0, 0),
                Vec::new(),
            ),
            (source_read_response_control(4, 5, 0), vec![0; 5]),
            (source_read_response_control(4, 3, 0), vec![0; 4]),
            (
                source_read_response_control(
                    RUNTIME_SOURCE_READ_MAX_BYTES,
                    RUNTIME_SOURCE_READ_MAX_BYTES,
                    0,
                ),
                vec![0; RUNTIME_SOURCE_READ_MAX_BYTES as usize + 1],
            ),
            (source_read_response_control(4, 1, 5), vec![0]),
        ] {
            assert!(RuntimeSourceReadHostResponse::decode_binary(12, &control, &payload).is_err());
        }
    }

    #[test]
    fn open_prepares_without_consuming_identity_and_publishes_only_after_validation() {
        let mut files = RuntimeFiles::default();
        files.register_snapshot(manifest()).unwrap();
        let SourceOpenOutcome::Opened(prepared) = files
            .prepare_open_with_disposition(
                b"Data\\ARC.gdb\0",
                b"C:\\Vine\0",
                0x8000_0000,
                0,
                3,
                0x0800_0020,
            )
            .unwrap()
        else {
            panic!("known source path must open");
        };
        assert!(files.open_files.is_empty());
        assert_eq!(files.next_file_id, 1);
        assert_eq!(prepared.file_id(), RuntimeSourceFileId(1));
        let opened = files.open_validated(prepared);
        assert_eq!(files.open_file(opened).unwrap().id, opened);
        assert_eq!(files.next_file_id, 2);
        assert_eq!(files.open_files.len(), 1);
        assert_eq!(files.open_file(opened).unwrap().share_mode, 0);
        assert!(matches!(
            files
                .prepare_open_with_disposition(
                    b"data\\arc.GDB\0",
                    b"C:\\Vine\0",
                    0x8000_0000,
                    1,
                    3,
                    0x0800_0020,
                )
                .unwrap(),
            SourceOpenOutcome::SharingViolation
        ));
        assert_eq!(files.open_files.len(), 1);
        assert_eq!(files.next_file_id, 2);
        assert_eq!(files.open_file(opened).unwrap().cursor, 0);
        files.validate_retirement(opened).unwrap();
        files.retire(opened);
        assert!(files.open_files.is_empty());
    }
}
