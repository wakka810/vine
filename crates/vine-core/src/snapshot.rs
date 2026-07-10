use sha2::{Digest, Sha256};

use crate::pe32::{LoaderError, Pe32Limits};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleIdentity {
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug)]
pub struct ModuleSnapshot {
    bytes: Box<[u8]>,
    sha256: [u8; 32],
}

impl ModuleSnapshot {
    pub fn from_bytes(bytes: Vec<u8>, limits: &Pe32Limits) -> Result<Self, LoaderError> {
        let byte_count = u64::try_from(bytes.len()).map_err(|_| {
            LoaderError::resource_limit(
                "module-snapshot",
                "source-length-not-representable",
                "source length does not fit the loader's 64-bit accounting field",
            )
        })?;
        if byte_count > limits.max_file_bytes {
            return Err(LoaderError::resource_limit(
                "module-snapshot",
                "source-file-limit-exceeded",
                format!(
                    "source has {byte_count} bytes; limit is {} bytes",
                    limits.max_file_bytes
                ),
            ));
        }

        let sha256 = digest(&bytes);
        Ok(Self {
            bytes: bytes.into_boxed_slice(),
            sha256,
        })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn identity(&self) -> ModuleIdentity {
        ModuleIdentity {
            bytes: self.bytes.len() as u64,
            sha256: hex_digest(&self.sha256),
        }
    }

    pub fn verify_integrity(&self) -> Result<(), LoaderError> {
        if digest(&self.bytes) != self.sha256 {
            return Err(LoaderError::runtime_defect(
                "module-snapshot",
                "immutable-snapshot-hash-mismatch",
                "owned module bytes no longer match their import-time SHA-256",
            ));
        }
        Ok(())
    }

    pub fn verify_expected_sha256(&self, expected: &str) -> Result<(), LoaderError> {
        if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(LoaderError::invalid_input(
                "module-snapshot",
                "invalid-expected-sha256",
                "expected SHA-256 must contain exactly 64 hexadecimal characters",
            ));
        }

        let actual = hex_digest(&self.sha256);
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(LoaderError::invalid_input(
                "module-snapshot",
                "source-fingerprint-mismatch",
                format!(
                    "source SHA-256 is {actual}, expected {}",
                    expected.to_ascii_lowercase()
                ),
            ));
        }
        Ok(())
    }
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hex_digest(digest: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(64);
    for byte in digest {
        value.push(char::from(HEX[usize::from(byte >> 4)]));
        value.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    value
}
