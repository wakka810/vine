use std::collections::BTreeSet;

use crate::memory::{GuestAddress, MemoryOwnerId};
use crate::pe32::{ImportTarget, LoaderError, Rva};

pub const IMPORT_THUNK_ARENA_BASE: GuestAddress = GuestAddress(0x7f00_0000);
pub const IMPORT_THUNK_ARENA_SIZE: u64 = 0x1_0000;
pub const IMPORT_THUNK_SLOT_SIZE: u32 = 0x10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ImportThunkId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportThunkDisposition {
    DeclaredRuntimeExport,
    UnsupportedTrap,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeExport {
    Name { module: String, name: String },
    Ordinal { module: String, ordinal: u16 },
}

impl RuntimeExport {
    pub fn named(module: impl Into<String>, name: impl Into<String>) -> Self {
        Self::Name {
            module: module.into(),
            name: name.into(),
        }
    }

    pub fn ordinal(module: impl Into<String>, ordinal: u16) -> Self {
        Self::Ordinal {
            module: module.into(),
            ordinal,
        }
    }

    fn module(&self) -> &str {
        match self {
            Self::Name { module, .. } | Self::Ordinal { module, .. } => module,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeExportCatalog {
    exports: Vec<RuntimeExport>,
}

impl RuntimeExportCatalog {
    pub fn new(exports: Vec<RuntimeExport>) -> Result<Self, LoaderError> {
        let mut keys = BTreeSet::new();
        for export in &exports {
            validate_runtime_export(export)?;
            let key = match export {
                RuntimeExport::Name { module, name } => {
                    RuntimeExport::named(module.to_ascii_lowercase(), name.clone())
                }
                RuntimeExport::Ordinal { module, ordinal } => {
                    RuntimeExport::ordinal(module.to_ascii_lowercase(), *ordinal)
                }
            };
            if !keys.insert(key) {
                return Err(LoaderError::new(
                    "runtime export catalog contains the same module and symbol more than once",
                ));
            }
        }
        Ok(Self { exports })
    }

    pub fn resolve_with_index(
        &self,
        module: &str,
        target: &ImportTarget,
    ) -> Option<(usize, &RuntimeExport)> {
        self.exports
            .iter()
            .enumerate()
            .find(|(_, export)| runtime_export_matches(export, module, target))
    }

    pub fn exports(&self) -> &[RuntimeExport] {
        &self.exports
    }

    pub fn len(&self) -> usize {
        self.exports.len()
    }

    pub fn is_empty(&self) -> bool {
        self.exports.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportThunk {
    pub id: ImportThunkId,
    pub address: GuestAddress,
    pub requester_owner: MemoryOwnerId,
    pub module: String,
    pub iat_rva: Rva,
    pub iat_address: GuestAddress,
    pub original_iat_value: u32,
    pub disposition: ImportThunkDisposition,
    pub target: ImportTarget,
}

fn validate_runtime_export(export: &RuntimeExport) -> Result<(), LoaderError> {
    let module = export.module();
    if module.is_empty()
        || module.len() > 260
        || !module.is_ascii()
        || module.as_bytes().contains(&0)
    {
        return Err(LoaderError::new(
            "runtime export module must be nonempty bounded ASCII without NUL bytes",
        ));
    }
    if let RuntimeExport::Name { name, .. } = export
        && (name.is_empty()
            || name.len() > 1_024
            || !name.is_ascii()
            || name.as_bytes().contains(&0))
    {
        return Err(LoaderError::new(
            "runtime export name must be nonempty bounded ASCII without NUL bytes",
        ));
    }
    Ok(())
}

fn runtime_export_matches(export: &RuntimeExport, module: &str, target: &ImportTarget) -> bool {
    match (export, target) {
        (
            RuntimeExport::Name {
                module: export_module,
                name: export_name,
            },
            ImportTarget::Name { name, .. },
        ) => export_module.eq_ignore_ascii_case(module) && export_name == name,
        (
            RuntimeExport::Ordinal {
                module: export_module,
                ordinal: export_ordinal,
            },
            ImportTarget::Ordinal { ordinal },
        ) => export_module.eq_ignore_ascii_case(module) && export_ordinal == ordinal,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_keys_fold_modules_but_preserve_names_and_ordinals() {
        assert!(
            RuntimeExportCatalog::new(vec![
                RuntimeExport::named("KERNEL32.DLL", "GetLastError"),
                RuntimeExport::named("kernel32.dll", "GetLastError"),
            ])
            .is_err()
        );
        assert!(
            RuntimeExportCatalog::new(vec![
                RuntimeExport::ordinal("KERNEL32.DLL", 7),
                RuntimeExport::ordinal("kernel32.dll", 7),
            ])
            .is_err()
        );
        let catalog = RuntimeExportCatalog::new(vec![
            RuntimeExport::named("kernel32.dll", "7"),
            RuntimeExport::ordinal("kernel32.dll", 7),
            RuntimeExport::named("kernel32.dll", "Entry"),
            RuntimeExport::named("kernel32.dll", "entry"),
        ])
        .unwrap();
        assert_eq!(
            catalog
                .resolve_with_index("KERNEL32.DLL", &ImportTarget::Ordinal { ordinal: 7 })
                .unwrap()
                .0,
            1
        );
        assert_eq!(
            catalog
                .resolve_with_index(
                    "kernel32.dll",
                    &ImportTarget::Name {
                        name_rva: Rva(0),
                        hint: 0,
                        name: "entry".to_owned()
                    }
                )
                .unwrap()
                .0,
            3
        );
    }
}
