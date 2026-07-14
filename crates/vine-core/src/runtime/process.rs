use serde::Serialize;

use super::{
    RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING, RUNTIME_PROCESS_IMAGE_PATH_A_MAX_BYTES,
    RuntimeProcessImagePathA, RuntimeStateError,
};

const PROCESS_IMAGE_DIRECTORY_AUTHORITY: &str = "runtime-process-image-directory-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCurrentDirectoryA {
    pub source_authority: String,
    pub bytes_with_nul: Vec<u8>,
    pub encoding: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeProcessDirectories {
    current_directory_a: RuntimeCurrentDirectoryA,
    current_directory_generation: u64,
    #[serde(skip)]
    process_image_directory_a: RuntimeCurrentDirectoryA,
    #[serde(skip)]
    process_image_directory_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSetCurrentDirectoryAPlan {
    previous: RuntimeCurrentDirectoryA,
    previous_generation: u64,
    next: RuntimeCurrentDirectoryA,
    next_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedDirectoryPath {
    bytes_with_nul: Vec<u8>,
    identity_key: Vec<u8>,
}

impl RuntimeProcessDirectories {
    pub(super) fn new(
        mut initial: RuntimeCurrentDirectoryA,
        image_path: &RuntimeProcessImagePathA,
    ) -> Result<Self, RuntimeStateError> {
        validate_path_authority(&initial.source_authority, &initial.encoding)?;
        let normalized_initial = normalize_absolute_directory_path(&initial.bytes_with_nul)?;
        initial.bytes_with_nul = normalized_initial.bytes_with_nul;

        validate_path_authority(&image_path.source_authority, &image_path.encoding)?;
        let image_directory = process_image_directory(image_path)?;
        let image_directory_key =
            normalize_absolute_directory_path(&image_directory.bytes_with_nul)?.identity_key;
        Ok(Self {
            current_directory_a: initial,
            current_directory_generation: 0,
            process_image_directory_a: image_directory,
            process_image_directory_key: image_directory_key,
        })
    }

    pub(super) fn current_directory_a(&self) -> &RuntimeCurrentDirectoryA {
        &self.current_directory_a
    }

    pub(super) const fn current_directory_generation(&self) -> u64 {
        self.current_directory_generation
    }

    pub(super) fn is_process_image_directory(
        &self,
        requested: &[u8],
    ) -> Result<bool, RuntimeStateError> {
        Ok(normalize_absolute_directory_path(requested)?.identity_key
            == self.process_image_directory_key)
    }

    pub(super) fn plan_process_image_directory(
        &self,
        requested: &[u8],
    ) -> Result<RuntimeSetCurrentDirectoryAPlan, RuntimeStateError> {
        if !self.is_process_image_directory(requested)? {
            return Err(RuntimeStateError::not_found(
                "runtime-process-directory",
                "process-image-directory-not-registered",
                "requested current directory is not the registered process image directory",
            ));
        }
        let next_generation = self
            .current_directory_generation
            .checked_add(1)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-process-directory",
                    "current-directory-generation-overflow",
                    "process current-directory generation overflowed",
                )
            })?;
        Ok(RuntimeSetCurrentDirectoryAPlan {
            previous: self.current_directory_a.clone(),
            previous_generation: self.current_directory_generation,
            next: self.process_image_directory_a.clone(),
            next_generation,
        })
    }

    pub(super) fn commit(
        &mut self,
        plan: RuntimeSetCurrentDirectoryAPlan,
    ) -> Result<&RuntimeCurrentDirectoryA, RuntimeStateError> {
        if self.current_directory_generation != plan.previous_generation
            || self.current_directory_a != plan.previous
        {
            return Err(RuntimeStateError::conflict(
                "runtime-process-directory",
                "current-directory-plan-stale",
                "process current-directory state changed after preflight",
            ));
        }
        self.current_directory_a = plan.next;
        self.current_directory_generation = plan.next_generation;
        Ok(&self.current_directory_a)
    }
}

fn validate_path_authority(
    source_authority: &str,
    encoding: &str,
) -> Result<(), RuntimeStateError> {
    if source_authority.is_empty() || encoding != RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING {
        return Err(RuntimeStateError::invalid_input(
            "runtime-process-directory",
            "current-directory-authority-invalid",
            "process directory paths require a nonempty authority and Windows-932 encoding",
        ));
    }
    Ok(())
}

fn process_image_directory(
    image_path: &RuntimeProcessImagePathA,
) -> Result<RuntimeCurrentDirectoryA, RuntimeStateError> {
    let normalized = normalize_absolute_directory_path(&image_path.bytes_with_nul)?;
    let body = &normalized.bytes_with_nul[..normalized.bytes_with_nul.len() - 1];
    let separator = last_separator_outside_windows_932(body).ok_or_else(|| {
        RuntimeStateError::invalid_input(
            "runtime-process-directory",
            "process-image-directory-missing",
            "process image path has no DOS directory component",
        )
    })?;
    if separator + 1 >= body.len() {
        return Err(RuntimeStateError::invalid_input(
            "runtime-process-directory",
            "process-image-name-missing",
            "process image path must end in a file name",
        ));
    }
    let directory_end = if separator == 2 { 3 } else { separator };
    let mut bytes_with_nul = body[..directory_end].to_vec();
    bytes_with_nul.push(0);
    let normalized_directory = normalize_absolute_directory_path(&bytes_with_nul)?;
    Ok(RuntimeCurrentDirectoryA {
        source_authority: PROCESS_IMAGE_DIRECTORY_AUTHORITY.to_owned(),
        bytes_with_nul: normalized_directory.bytes_with_nul,
        encoding: RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
    })
}

fn normalize_absolute_directory_path(
    bytes_with_nul: &[u8],
) -> Result<NormalizedDirectoryPath, RuntimeStateError> {
    if bytes_with_nul.len() < 2
        || bytes_with_nul.len() > RUNTIME_PROCESS_IMAGE_PATH_A_MAX_BYTES
        || bytes_with_nul.last() != Some(&0)
        || bytes_with_nul[..bytes_with_nul.len() - 1].contains(&0)
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-process-directory",
            "current-directory-path-shape-invalid",
            "process current directory requires one bounded trailing-NUL ANSI path",
        ));
    }
    let body = &bytes_with_nul[..bytes_with_nul.len() - 1];
    if body.len() < 3 || !body[0].is_ascii_alphabetic() || body[1] != b':' || !is_separator(body[2])
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-process-directory",
            "current-directory-path-not-absolute",
            "process current-directory subset requires an absolute DOS drive path",
        ));
    }

    let mut components = Vec::<Vec<u8>>::new();
    let mut component = Vec::new();
    let mut index = 3;
    while index < body.len() {
        let byte = body[index];
        if is_separator(byte) {
            if !component.is_empty() {
                validate_component(&component)?;
                components.push(std::mem::take(&mut component));
            }
            index += 1;
            continue;
        }
        if is_windows_932_lead_byte(byte) {
            let Some(&trail) = body.get(index + 1) else {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-process-directory",
                    "current-directory-windows-932-sequence-invalid",
                    "process current-directory path ends in a Windows-932 lead byte",
                ));
            };
            if !is_windows_932_trail_byte(trail) {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-process-directory",
                    "current-directory-windows-932-sequence-invalid",
                    "process current-directory path contains an invalid Windows-932 trail byte",
                ));
            }
            component.extend_from_slice(&[byte, trail]);
            index += 2;
            continue;
        }
        if byte < 0x20
            || byte == 0x7f
            || matches!(byte, b'"' | b'<' | b'>' | b'|' | b'?' | b'*' | b':')
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-directory",
                "current-directory-character-invalid",
                "process current-directory path contains a disallowed DOS path character",
            ));
        }
        component.push(byte);
        index += 1;
    }
    if !component.is_empty() {
        validate_component(&component)?;
        components.push(component);
    }

    let mut normalized = vec![body[0].to_ascii_uppercase(), b':', b'\\'];
    for (component_index, component) in components.iter().enumerate() {
        if component_index != 0 {
            normalized.push(b'\\');
        }
        normalized.extend_from_slice(component);
    }
    normalized.push(0);
    let identity_key = windows_932_ascii_fold(&normalized[..normalized.len() - 1]);
    Ok(NormalizedDirectoryPath {
        bytes_with_nul: normalized,
        identity_key,
    })
}

fn validate_component(component: &[u8]) -> Result<(), RuntimeStateError> {
    if component == b"." || component == b".." {
        return Err(RuntimeStateError::invalid_input(
            "runtime-process-directory",
            "current-directory-relative-component-unsupported",
            "process current-directory subset does not resolve dot components without a VFS",
        ));
    }
    Ok(())
}

fn last_separator_outside_windows_932(bytes: &[u8]) -> Option<usize> {
    let mut last = None;
    let mut index = 0;
    while index < bytes.len() {
        if is_windows_932_lead_byte(bytes[index])
            && bytes
                .get(index + 1)
                .is_some_and(|byte| is_windows_932_trail_byte(*byte))
        {
            index += 2;
        } else {
            if is_separator(bytes[index]) {
                last = Some(index);
            }
            index += 1;
        }
    }
    last
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

const fn is_separator(byte: u8) -> bool {
    matches!(byte, b'\\' | b'/')
}

const fn is_windows_932_lead_byte(byte: u8) -> bool {
    matches!(byte, 0x81..=0x9f | 0xe0..=0xfc)
}

const fn is_windows_932_trail_byte(byte: u8) -> bool {
    matches!(byte, 0x40..=0x7e | 0x80..=0xfc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current_directory(bytes: &[u8]) -> RuntimeCurrentDirectoryA {
        RuntimeCurrentDirectoryA {
            source_authority: "process-test-bootstrap".to_owned(),
            bytes_with_nul: bytes.to_vec(),
            encoding: RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        }
    }

    fn image_path(bytes: &[u8]) -> RuntimeProcessImagePathA {
        RuntimeProcessImagePathA {
            source_authority: "process-test-image".to_owned(),
            bytes_with_nul: bytes.to_vec(),
            encoding: RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        }
    }

    #[test]
    fn current_directory_plan_is_copy_owned_and_commits_only_after_preflight() {
        let mut directories = RuntimeProcessDirectories::new(
            current_directory(b"D:\\Launcher\0"),
            &image_path(b"C:\\Vine\\BGI.exe\0"),
        )
        .unwrap();
        let before = directories.clone();
        let mut request = b"c:/VINE/\0".to_vec();
        let plan = directories.plan_process_image_directory(&request).unwrap();
        request.fill(b'X');
        assert_eq!(directories, before);
        assert_eq!(
            directories.commit(plan).unwrap(),
            &current_directory(b"C:\\Vine\0").with_authority(PROCESS_IMAGE_DIRECTORY_AUTHORITY)
        );
        assert_eq!(directories.current_directory_generation(), 1);
    }

    #[test]
    fn rejected_and_stale_plans_preserve_directory_state() {
        let mut directories = RuntimeProcessDirectories::new(
            current_directory(b"C:\\Vine\0"),
            &image_path(b"C:\\Vine\\BGI.exe\0"),
        )
        .unwrap();
        let before = directories.clone();
        assert_eq!(
            directories
                .plan_process_image_directory(b"C:\\Other\0")
                .unwrap_err()
                .code,
            "process-image-directory-not-registered"
        );
        assert_eq!(
            directories
                .plan_process_image_directory(b"..\0")
                .unwrap_err()
                .code,
            "current-directory-path-not-absolute"
        );
        assert_eq!(directories, before);

        let stale = directories
            .plan_process_image_directory(b"C:\\Vine\\\0")
            .unwrap();
        let first = directories
            .plan_process_image_directory(b"C:\\Vine\0")
            .unwrap();
        directories.commit(first).unwrap();
        let after_first = directories.clone();
        assert_eq!(
            directories.commit(stale).unwrap_err().code,
            "current-directory-plan-stale"
        );
        assert_eq!(directories, after_first);
    }

    #[test]
    fn windows_932_trail_backslash_is_not_a_directory_separator() {
        let image = image_path(&[
            b'C', b':', b'\\', 0x81, 0x5c, b'\\', b'B', b'G', b'I', b'.', b'e', b'x', b'e', 0,
        ]);
        let directories = RuntimeProcessDirectories::new(
            current_directory(&[b'C', b':', b'\\', 0x81, 0x5c, 0]),
            &image,
        )
        .unwrap();
        assert!(
            directories
                .is_process_image_directory(&[b'c', b':', b'/', 0x81, 0x5c, b'/', 0])
                .unwrap()
        );
    }

    trait WithAuthority {
        fn with_authority(self, authority: &str) -> Self;
    }

    impl WithAuthority for RuntimeCurrentDirectoryA {
        fn with_authority(mut self, authority: &str) -> Self {
            self.source_authority = authority.to_owned();
            self
        }
    }
}
