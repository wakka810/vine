use std::collections::BTreeSet;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::materialize::{ImageMaterialization, UnboundImport};
use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, GuestMemoryErrorClass,
    GuestRange, MemoryInvalidation, MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind,
    PageProtection,
};
use crate::pe32::{
    ImportSymbolPlan, ImportTarget, LoaderError, LoaderErrorClass, LoaderIngressReport, Rva,
    UnsupportedPoint,
};
use crate::snapshot::ModuleIdentity;

pub const IMPORT_THUNK_ARENA_BASE: GuestAddress = GuestAddress(0x7f00_0000);
pub const IMPORT_THUNK_ARENA_SIZE: u64 = 0x1_0000;
pub const IMPORT_THUNK_SLOT_SIZE: u32 = 0x10;
const IMPORT_THUNK_CAPACITY: usize =
    (IMPORT_THUNK_ARENA_SIZE / IMPORT_THUNK_SLOT_SIZE as u64) as usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct ImportThunkId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImportThunkDisposition {
    DeclaredRuntimeExport,
    UnsupportedTrap,
}

impl ImportThunkDisposition {
    fn is_declared_runtime_export(&self) -> bool {
        *self == Self::DeclaredRuntimeExport
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
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
            let key = runtime_export_key(export);
            if !keys.insert(key) {
                return Err(LoaderError::invalid_input(
                    "import-binding",
                    "runtime-export-duplicate",
                    "runtime export catalog contains the same module and symbol more than once",
                ));
            }
        }
        Ok(Self { exports })
    }

    pub fn supports(&self, module: &str, target: &ImportTarget) -> bool {
        self.exports
            .iter()
            .any(|export| runtime_export_matches(export, module, target))
    }

    pub fn len(&self) -> usize {
        self.exports.len()
    }

    pub fn is_empty(&self) -> bool {
        self.exports.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportThunk {
    pub id: ImportThunkId,
    pub address: GuestAddress,
    pub requester_owner: MemoryOwnerId,
    pub module: String,
    pub iat_rva: Rva,
    pub iat_address: GuestAddress,
    pub original_iat_value: u32,
    #[serde(skip_serializing_if = "ImportThunkDisposition::is_declared_runtime_export")]
    pub disposition: ImportThunkDisposition,
    #[serde(flatten)]
    pub target: ImportTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportThunkArena {
    pub owner: MemoryOwnerId,
    pub range: GuestRange,
    pub committed_pages: usize,
    pub slot_size: u32,
    pub slot_capacity: usize,
    pub bound_slots: usize,
    pub content_sha256: String,
    pub owner_sealed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportBinding {
    pub source: ModuleIdentity,
    pub image_owner: MemoryOwnerId,
    pub image_load_base: GuestAddress,
    pub total_imports: usize,
    pub bound_imports: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub unsupported_trap_imports: usize,
    pub remaining_imports: usize,
    pub complete: bool,
    pub image_owner_sealed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arena: Option<ImportThunkArena>,
    pub thunks: Vec<ImportThunk>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_unresolved_import: Option<UnboundImport>,
    pub invalidation: MemoryInvalidation,
    pub stop: UnsupportedPoint,
}

impl ImportBinding {
    pub fn thunk_at(&self, address: GuestAddress) -> Option<&ImportThunk> {
        self.thunks.iter().find(|thunk| thunk.address == address)
    }
}

pub fn bind_pe32_imports(
    memory: &mut GuestMemory,
    loader: &LoaderIngressReport,
    image: &ImageMaterialization,
    catalog: &RuntimeExportCatalog,
) -> Result<ImportBinding, LoaderError> {
    bind_pe32_imports_internal(memory, loader, image, catalog, MissingImportPolicy::Stop)
}

pub fn bind_pe32_imports_with_unsupported_traps(
    memory: &mut GuestMemory,
    loader: &LoaderIngressReport,
    image: &ImageMaterialization,
    catalog: &RuntimeExportCatalog,
) -> Result<ImportBinding, LoaderError> {
    bind_pe32_imports_internal(
        memory,
        loader,
        image,
        catalog,
        MissingImportPolicy::BindUnsupportedTrap,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MissingImportPolicy {
    Stop,
    BindUnsupportedTrap,
}

fn bind_pe32_imports_internal(
    memory: &mut GuestMemory,
    loader: &LoaderIngressReport,
    image: &ImageMaterialization,
    catalog: &RuntimeExportCatalog,
    missing_import_policy: MissingImportPolicy,
) -> Result<ImportBinding, LoaderError> {
    validate_binding_input(memory, loader, image)?;
    let total_imports = loader
        .load_plan
        .imports
        .iter()
        .try_fold(0usize, |count, module| {
            count
                .checked_add(module.symbols.len())
                .ok_or_else(import_binding_accounting_overflow)
        })?;
    let mut planned = Vec::new();
    let mut first_unresolved_import = None;

    'modules: for module in &loader.load_plan.imports {
        for symbol in &module.symbols {
            let iat_address =
                add_rva(image.load_base, symbol.iat_rva, "iat-slot-address-overflow")?;
            let current = read_initialization_u32(memory, image.layout.owner, iat_address)?;
            if current != symbol.original_iat_value {
                return Err(LoaderError::invalid_input(
                    "import-binding",
                    "iat-slot-not-original",
                    format!(
                        "IAT slot {:#010x} no longer contains its planned original value",
                        iat_address.0
                    ),
                ));
            }
            let disposition = if catalog.supports(&module.name, &symbol.target) {
                ImportThunkDisposition::DeclaredRuntimeExport
            } else if missing_import_policy == MissingImportPolicy::BindUnsupportedTrap {
                ImportThunkDisposition::UnsupportedTrap
            } else {
                first_unresolved_import =
                    Some(unbound_import(&module.name, symbol, image.load_base)?);
                break 'modules;
            };
            planned.push((module.name.as_str(), symbol, iat_address, disposition));
        }
    }

    if planned.len() > IMPORT_THUNK_CAPACITY {
        return Err(LoaderError::resource_limit(
            "import-binding",
            "import-thunk-capacity-exceeded",
            format!(
                "{} bound imports exceed the version 1 thunk capacity {IMPORT_THUNK_CAPACITY}",
                planned.len()
            ),
        ));
    }
    let committed_bytes = if planned.is_empty() {
        0
    } else {
        let used_bytes = planned
            .len()
            .checked_mul(IMPORT_THUNK_SLOT_SIZE as usize)
            .ok_or_else(import_binding_accounting_overflow)?;
        align_up_to_page(used_bytes)?
    };
    let required_pages = committed_bytes / GUEST_PAGE_SIZE as usize;
    let stats_before_binding = memory.stats();
    let available_pages = memory
        .limits()
        .max_physical_pages
        .saturating_sub(stats_before_binding.physical_pages);
    if required_pages > available_pages {
        return Err(LoaderError::resource_limit(
            "import-binding",
            "import-thunk-physical-page-quota-exceeded",
            format!(
                "import thunk arena needs {required_pages} physical pages; {available_pages} remain under quota"
            ),
        ));
    }

    let mut thunks = Vec::with_capacity(planned.len());
    let arena = if planned.is_empty() {
        None
    } else {
        let arena_owner = memory
            .create_owner(MemoryOwnerKind::Runtime)
            .map_err(memory_error)?;
        let arena_range = match memory.reserve_fixed(
            arena_owner,
            MemoryRegionKind::Runtime,
            IMPORT_THUNK_ARENA_BASE,
            IMPORT_THUNK_ARENA_SIZE,
        ) {
            Ok(range) => range,
            Err(error) => {
                memory.destroy_owner(arena_owner).map_err(memory_error)?;
                return Err(memory_error(error));
            }
        };
        let committed_range = GuestRange::new(
            IMPORT_THUNK_ARENA_BASE,
            u64::try_from(committed_bytes).map_err(|_| {
                LoaderError::resource_limit(
                    "import-binding",
                    "thunk-commit-size-not-representable",
                    "thunk commit size does not fit 64-bit guest range accounting",
                )
            })?,
        )
        .map_err(memory_error)?;
        memory
            .commit(
                arena_owner,
                committed_range,
                PageProtection {
                    read: true,
                    write: false,
                    execute: true,
                },
            )
            .map_err(memory_error)?;

        fill_thunk_pages(memory, arena_owner, committed_bytes)?;
        for (index, (module, symbol, iat_address, disposition)) in planned.iter().enumerate() {
            let id_value = u32::try_from(index + 1).map_err(|_| {
                LoaderError::resource_limit(
                    "import-binding",
                    "import-thunk-id-not-representable",
                    "import thunk identifier does not fit 32 bits",
                )
            })?;
            let slot_offset = u32::try_from(index)
                .map_err(|_| import_binding_accounting_overflow())?
                .checked_mul(IMPORT_THUNK_SLOT_SIZE)
                .ok_or_else(import_binding_accounting_overflow)?;
            let address = GuestAddress(
                IMPORT_THUNK_ARENA_BASE
                    .0
                    .checked_add(slot_offset)
                    .ok_or_else(import_binding_accounting_overflow)?,
            );
            memory
                .write_initialization(arena_owner, address, &[0x0f, 0x0b])
                .map_err(memory_error)?;
            memory
                .write_initialization(image.layout.owner, *iat_address, &address.0.to_le_bytes())
                .map_err(memory_error)?;
            thunks.push(ImportThunk {
                id: ImportThunkId(id_value),
                address,
                requester_owner: image.layout.owner,
                module: (*module).to_owned(),
                iat_rva: symbol.iat_rva,
                iat_address: *iat_address,
                original_iat_value: symbol.original_iat_value,
                disposition: *disposition,
                target: symbol.target.clone(),
            });
        }
        verify_bound_iat(memory, image.layout.owner, &thunks)?;
        let content_sha256 = hash_initialization_range(
            memory,
            arena_owner,
            IMPORT_THUNK_ARENA_BASE,
            committed_bytes,
        )?;
        memory.seal_owner(arena_owner).map_err(memory_error)?;
        Some(ImportThunkArena {
            owner: arena_owner,
            range: arena_range,
            committed_pages: committed_bytes / GUEST_PAGE_SIZE as usize,
            slot_size: IMPORT_THUNK_SLOT_SIZE,
            slot_capacity: IMPORT_THUNK_CAPACITY,
            bound_slots: thunks.len(),
            content_sha256,
            owner_sealed: true,
        })
    };

    let complete = first_unresolved_import.is_none();
    if complete {
        memory
            .seal_owner(image.layout.owner)
            .map_err(memory_error)?;
    }
    let pending = memory.stats().pending_invalidation.ok_or_else(|| {
        LoaderError::runtime_defect(
            "import-binding",
            "binding-invalidation-missing",
            "import binding completed without a pending memory invalidation",
        )
    })?;
    let bound_imports = thunks.len();
    let unsupported_trap_imports = thunks
        .iter()
        .filter(|thunk| thunk.disposition == ImportThunkDisposition::UnsupportedTrap)
        .count();
    let remaining_imports = total_imports
        .checked_sub(bound_imports)
        .ok_or_else(import_binding_accounting_overflow)?;
    let stop = if complete && unsupported_trap_imports != 0 {
        UnsupportedPoint {
            stage: "thunk-dispatch",
            code: "unsupported-import-trap-dispatch-pending",
            message: "all planned imports are bound and the image is sealed; catalog-missing imports point to identity-preserving UD2 traps that return no value, and guest execution has not been attempted",
        }
    } else if complete {
        UnsupportedPoint {
            stage: "process-bootstrap",
            code: "peb-teb-process-parameters-unimplemented",
            message: "all planned imports are bound to runtime-owned thunks and the image is sealed; PEB/TEB/process parameters, initial stack, thunk dispatch, and guest execution were not attempted",
        }
    } else {
        UnsupportedPoint {
            stage: "import-resolution",
            code: "import-symbol-unimplemented",
            message: "a planned import has no runtime-owned export endpoint; later imports, image sealing, process bootstrap, thunk dispatch, and guest execution were not attempted",
        }
    };

    Ok(ImportBinding {
        source: loader.source.clone(),
        image_owner: image.layout.owner,
        image_load_base: image.load_base,
        total_imports,
        bound_imports,
        unsupported_trap_imports,
        remaining_imports,
        complete,
        image_owner_sealed: complete,
        arena,
        thunks,
        first_unresolved_import,
        invalidation: pending,
        stop,
    })
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

fn validate_binding_input(
    memory: &GuestMemory,
    loader: &LoaderIngressReport,
    image: &ImageMaterialization,
) -> Result<(), LoaderError> {
    if loader.schema != "vine.pe32-loader-ingress" || loader.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "import-binding",
            "loader-schema-unsupported",
            "import binding requires vine.pe32-loader-ingress schema version 1",
        ));
    }
    if loader.source != image.source || loader.source != image.layout.source {
        return Err(LoaderError::invalid_input(
            "import-binding",
            "materialized-source-identity-mismatch",
            "load plan and materialized image do not share the same source identity",
        ));
    }
    if image.layout.image_range.start() != image.load_base
        || image.layout.image_range.byte_length() != u64::from(loader.load_plan.image.size_of_image)
    {
        return Err(LoaderError::invalid_input(
            "import-binding",
            "materialized-image-range-mismatch",
            "materialized image load base and range do not match the load plan",
        ));
    }
    if image.layout.owner_sealed {
        return Err(LoaderError::invalid_input(
            "import-binding",
            "materialized-image-already-sealed",
            "IAT binding requires an unsealed materialized image owner",
        ));
    }
    if !loader.load_plan.imports.is_empty()
        && (image.stop.stage != "import-binding" || image.stop.code != "iat-binding-unimplemented")
    {
        return Err(LoaderError::invalid_input(
            "import-binding",
            "materialized-image-stop-mismatch",
            "materialized image is not stopped at the import-binding boundary",
        ));
    }
    let stats = memory.stats();
    if stats.pending_invalidation != Some(image.layout.invalidation) {
        return Err(LoaderError::invalid_input(
            "import-binding",
            "materialized-memory-generation-mismatch",
            "guest memory is not at the materialized image invalidation generation",
        ));
    }
    Ok(())
}

fn fill_thunk_pages(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
    byte_length: usize,
) -> Result<(), LoaderError> {
    let chunk_limit = memory.limits().max_access_bytes;
    let halt_bytes = vec![0xf4; chunk_limit.min(byte_length)];
    let mut offset = 0usize;
    while offset < byte_length {
        let chunk_length = (byte_length - offset).min(halt_bytes.len());
        let address = add_offset(
            IMPORT_THUNK_ARENA_BASE,
            offset,
            "thunk-page-fill-address-overflow",
        )?;
        memory
            .write_initialization(owner, address, &halt_bytes[..chunk_length])
            .map_err(memory_error)?;
        offset = offset
            .checked_add(chunk_length)
            .ok_or_else(import_binding_accounting_overflow)?;
    }
    Ok(())
}

fn verify_bound_iat(
    memory: &GuestMemory,
    image_owner: MemoryOwnerId,
    thunks: &[ImportThunk],
) -> Result<(), LoaderError> {
    for thunk in thunks {
        let actual = read_initialization_u32(memory, image_owner, thunk.iat_address)?;
        if actual != thunk.address.0 {
            return Err(LoaderError::runtime_defect(
                "import-binding",
                "bound-iat-verification-failed",
                format!(
                    "IAT slot {:#010x} does not contain owned thunk {:#010x}",
                    thunk.iat_address.0, thunk.address.0
                ),
            ));
        }
    }
    Ok(())
}

fn read_initialization_u32(
    memory: &GuestMemory,
    owner: MemoryOwnerId,
    address: GuestAddress,
) -> Result<u32, LoaderError> {
    let bytes = memory
        .read_initialization(owner, address, 4)
        .map_err(memory_error)?;
    let bytes: [u8; 4] = bytes.try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "import-binding",
            "iat-read-width-invalid",
            "preflighted IAT read did not return four bytes",
        )
    })?;
    Ok(u32::from_le_bytes(bytes))
}

fn hash_initialization_range(
    memory: &GuestMemory,
    owner: MemoryOwnerId,
    address: GuestAddress,
    byte_length: usize,
) -> Result<String, LoaderError> {
    let chunk_limit = memory.limits().max_access_bytes;
    let mut hasher = Sha256::new();
    let mut offset = 0usize;
    while offset < byte_length {
        let chunk_length = (byte_length - offset).min(chunk_limit);
        let chunk_address = add_offset(address, offset, "thunk-hash-address-overflow")?;
        let bytes = memory
            .read_initialization(owner, chunk_address, chunk_length)
            .map_err(memory_error)?;
        hasher.update(bytes);
        offset = offset
            .checked_add(chunk_length)
            .ok_or_else(import_binding_accounting_overflow)?;
    }
    Ok(hex_digest(&hasher.finalize()))
}

fn unbound_import(
    module: &str,
    symbol: &ImportSymbolPlan,
    load_base: GuestAddress,
) -> Result<UnboundImport, LoaderError> {
    Ok(UnboundImport {
        module: module.to_owned(),
        iat_rva: symbol.iat_rva,
        iat_address: add_rva(load_base, symbol.iat_rva, "unresolved-iat-address-overflow")?,
        original_iat_value: symbol.original_iat_value,
        target: symbol.target.clone(),
    })
}

fn validate_runtime_export(export: &RuntimeExport) -> Result<(), LoaderError> {
    let module = export.module();
    if module.is_empty()
        || module.len() > 260
        || !module.is_ascii()
        || module.as_bytes().contains(&0)
    {
        return Err(LoaderError::invalid_input(
            "import-binding",
            "runtime-export-module-invalid",
            "runtime export module must be nonempty bounded ASCII without NUL bytes",
        ));
    }
    if let RuntimeExport::Name { name, .. } = export
        && (name.is_empty()
            || name.len() > 1_024
            || !name.is_ascii()
            || name.as_bytes().contains(&0))
    {
        return Err(LoaderError::invalid_input(
            "import-binding",
            "runtime-export-name-invalid",
            "runtime export name must be nonempty bounded ASCII without NUL bytes",
        ));
    }
    Ok(())
}

fn runtime_export_key(export: &RuntimeExport) -> String {
    match export {
        RuntimeExport::Name { module, name } => {
            format!("{}!name:{name}", module.to_ascii_lowercase())
        }
        RuntimeExport::Ordinal { module, ordinal } => {
            format!("{}!ordinal:{ordinal}", module.to_ascii_lowercase())
        }
    }
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

fn align_up_to_page(value: usize) -> Result<usize, LoaderError> {
    let page_size = GUEST_PAGE_SIZE as usize;
    let adjusted = value
        .checked_add(page_size - 1)
        .ok_or_else(import_binding_accounting_overflow)?;
    Ok(adjusted / page_size * page_size)
}

fn add_rva(base: GuestAddress, rva: Rva, code: &'static str) -> Result<GuestAddress, LoaderError> {
    let address = base.0.checked_add(rva.0).ok_or_else(|| {
        LoaderError::invalid_input(
            "import-binding",
            code,
            "image load base plus IAT RVA overflows 32 bits",
        )
    })?;
    Ok(GuestAddress(address))
}

fn add_offset(
    base: GuestAddress,
    offset: usize,
    code: &'static str,
) -> Result<GuestAddress, LoaderError> {
    let offset = u32::try_from(offset).map_err(|_| {
        LoaderError::resource_limit(
            "import-binding",
            "thunk-offset-not-representable",
            "thunk arena offset does not fit 32 bits",
        )
    })?;
    let address = base.0.checked_add(offset).ok_or_else(|| {
        LoaderError::invalid_input(
            "import-binding",
            code,
            "thunk arena address overflows 32 bits",
        )
    })?;
    Ok(GuestAddress(address))
}

fn memory_error(error: GuestMemoryError) -> LoaderError {
    let class = match error.class {
        GuestMemoryErrorClass::ResourceLimit => LoaderErrorClass::ResourceLimit,
        GuestMemoryErrorClass::RuntimeDefect => LoaderErrorClass::RuntimeDefect,
        GuestMemoryErrorClass::InvalidRequest
        | GuestMemoryErrorClass::Conflict
        | GuestMemoryErrorClass::SynchronizationRequired
        | GuestMemoryErrorClass::Fault => LoaderErrorClass::InvalidInput,
    };
    LoaderError {
        class,
        stage: error.stage,
        code: error.code,
        message: error.message,
    }
}

fn import_binding_accounting_overflow() -> LoaderError {
    LoaderError::resource_limit(
        "import-binding",
        "import-binding-accounting-overflow",
        "import binding arithmetic exceeded the host accounting range",
    )
}

fn hex_digest(digest: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(digest.len() * 2);
    for byte in digest {
        value.push(char::from(HEX[usize::from(byte >> 4)]));
        value.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    value
}
