use sha2::{Digest, Sha256};

use crate::pe32::{LoaderError, Pe32Limits};

#[derive(Debug, Clone, PartialEq, Eq)]
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
            LoaderError::new("source length does not fit the loader's 64-bit accounting field")
        })?;
        if byte_count > limits.max_file_bytes {
            return Err(LoaderError::new(format!(
                "source has {byte_count} bytes; limit is {} bytes",
                limits.max_file_bytes
            )));
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
