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
use crate::runtime::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationLifetime,
    ProcessAddressAllocationPurpose, ProcessAddressAllocationRecord,
    ProcessAddressAllocationReleasePolicy, ProcessAddressMappingRequirements, RuntimeAddressRange,
    validate_process_address_mapping_allocation_for_memory,
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
        self.resolve(module, target).is_some()
    }

    pub fn resolve(&self, module: &str, target: &ImportTarget) -> Option<&RuntimeExport> {
        self.exports
            .iter()
            .find(|export| runtime_export_matches(export, module, target))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImportBindingPolicy {
    RequireDeclaredExport,
    BindUnsupportedTrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImportPlanDisposition {
    DeclaredRuntimeExport,
    UnsupportedTrap,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedImport {
    pub id: ImportThunkId,
    pub thunk_address: GuestAddress,
    pub requester_owner: MemoryOwnerId,
    pub module: String,
    pub iat_rva: Rva,
    pub iat_address: GuestAddress,
    pub original_iat_value: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog_endpoint: Option<RuntimeExport>,
    pub disposition: ImportPlanDisposition,
    #[serde(flatten)]
    pub target: ImportTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportBindingPlan {
    #[serde(skip_serializing)]
    pub address_allocation: ProcessAddressAllocationRecord,
    pub source: ModuleIdentity,
    pub image_owner: MemoryOwnerId,
    pub image_load_base: GuestAddress,
    pub memory_generation: u64,
    pub policy: ImportBindingPolicy,
    pub total_imports: usize,
    pub committed_bytes: usize,
    pub required_pages: usize,
    pub imports: Vec<PlannedImport>,
    pub unresolved_imports: Vec<UnboundImport>,
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportCommitStage {
    ArenaOwnerCreated,
    ArenaReserved,
    ArenaCommitted,
    ThunkPagesFilled,
    ThunkWritten(usize),
    IatWritten(usize),
    IatVerified,
    ArenaSealed,
    ImageSealed,
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
    pub unresolved_imports: Vec<UnboundImport>,
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
    address_allocation: &ProcessAddressAllocationRecord,
    loader: &LoaderIngressReport,
    image: &ImageMaterialization,
    catalog: &RuntimeExportCatalog,
) -> Result<ImportBinding, LoaderError> {
    let plan = plan_pe32_imports(memory, address_allocation, loader, image, catalog)?;
    bind_or_report_unresolved(memory, loader, image, plan)
}

pub fn bind_pe32_imports_with_unsupported_traps(
    memory: &mut GuestMemory,
    address_allocation: &ProcessAddressAllocationRecord,
    loader: &LoaderIngressReport,
    image: &ImageMaterialization,
    catalog: &RuntimeExportCatalog,
) -> Result<ImportBinding, LoaderError> {
    let plan = plan_pe32_imports_with_unsupported_traps(
        memory,
        address_allocation,
        loader,
        image,
        catalog,
    )?;
    commit_pe32_import_binding(memory, loader, image, &plan)
}

pub fn plan_pe32_imports(
    memory: &GuestMemory,
    address_allocation: &ProcessAddressAllocationRecord,
    loader: &LoaderIngressReport,
    image: &ImageMaterialization,
    catalog: &RuntimeExportCatalog,
) -> Result<ImportBindingPlan, LoaderError> {
    plan_pe32_imports_internal(
        memory,
        address_allocation,
        loader,
        image,
        catalog,
        ImportBindingPolicy::RequireDeclaredExport,
    )
}

pub fn plan_pe32_imports_with_unsupported_traps(
    memory: &GuestMemory,
    address_allocation: &ProcessAddressAllocationRecord,
    loader: &LoaderIngressReport,
    image: &ImageMaterialization,
    catalog: &RuntimeExportCatalog,
) -> Result<ImportBindingPlan, LoaderError> {
    plan_pe32_imports_internal(
        memory,
        address_allocation,
        loader,
        image,
        catalog,
        ImportBindingPolicy::BindUnsupportedTrap,
    )
}

fn plan_pe32_imports_internal(
    memory: &GuestMemory,
    address_allocation: &ProcessAddressAllocationRecord,
    loader: &LoaderIngressReport,
    image: &ImageMaterialization,
    catalog: &RuntimeExportCatalog,
    policy: ImportBindingPolicy,
) -> Result<ImportBindingPlan, LoaderError> {
    validate_import_mapping_allocation(address_allocation)?;
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
    if total_imports > IMPORT_THUNK_CAPACITY {
        return Err(LoaderError::resource_limit(
            "import-binding",
            "import-thunk-capacity-exceeded",
            format!(
                "{total_imports} planned imports exceed the version 1 thunk capacity {IMPORT_THUNK_CAPACITY}"
            ),
        ));
    }

    let mut imports = Vec::with_capacity(total_imports);
    let mut unresolved_imports = Vec::new();
    for module in &loader.load_plan.imports {
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
            let id = ImportThunkId(u32::try_from(imports.len() + 1).map_err(|_| {
                LoaderError::resource_limit(
                    "import-binding",
                    "import-thunk-id-not-representable",
                    "import thunk identifier does not fit 32 bits",
                )
            })?);
            let slot_offset = u32::try_from(imports.len())
                .map_err(|_| import_binding_accounting_overflow())?
                .checked_mul(IMPORT_THUNK_SLOT_SIZE)
                .ok_or_else(import_binding_accounting_overflow)?;
            let thunk_address = GuestAddress(
                IMPORT_THUNK_ARENA_BASE
                    .0
                    .checked_add(slot_offset)
                    .ok_or_else(import_binding_accounting_overflow)?,
            );
            let catalog_endpoint = catalog.resolve(&module.name, &symbol.target).cloned();
            let disposition = if catalog_endpoint.is_some() {
                ImportPlanDisposition::DeclaredRuntimeExport
            } else if policy == ImportBindingPolicy::BindUnsupportedTrap {
                ImportPlanDisposition::UnsupportedTrap
            } else {
                unresolved_imports.push(unbound_import(&module.name, symbol, image.load_base)?);
                ImportPlanDisposition::Unresolved
            };
            imports.push(PlannedImport {
                id,
                thunk_address,
                requester_owner: image.layout.owner,
                module: module.name.clone(),
                iat_rva: symbol.iat_rva,
                iat_address,
                original_iat_value: symbol.original_iat_value,
                catalog_endpoint,
                disposition,
                target: symbol.target.clone(),
            });
        }
    }

    let committed_bytes = if imports.is_empty() {
        0
    } else {
        let used_bytes = imports
            .len()
            .checked_mul(IMPORT_THUNK_SLOT_SIZE as usize)
            .ok_or_else(import_binding_accounting_overflow)?;
        align_up_to_page(used_bytes)?
    };
    let required_pages = committed_bytes / GUEST_PAGE_SIZE as usize;
    let complete = unresolved_imports.is_empty();
    if complete && !imports.is_empty() {
        memory
            .preflight_growth(1, 1, required_pages, "import-binding")
            .map_err(memory_error)?;
        memory
            .preflight_fixed_reservation(
                IMPORT_THUNK_ARENA_BASE,
                IMPORT_THUNK_ARENA_SIZE,
                "import-binding",
            )
            .map_err(memory_error)?;
    }

    Ok(ImportBindingPlan {
        address_allocation: address_allocation.clone(),
        source: loader.source.clone(),
        image_owner: image.layout.owner,
        image_load_base: image.load_base,
        memory_generation: memory.stats().generation,
        policy,
        total_imports,
        committed_bytes,
        required_pages,
        imports,
        unresolved_imports,
        complete,
    })
}

fn bind_or_report_unresolved(
    memory: &mut GuestMemory,
    loader: &LoaderIngressReport,
    image: &ImageMaterialization,
    plan: ImportBindingPlan,
) -> Result<ImportBinding, LoaderError> {
    if plan.complete {
        return commit_pe32_import_binding(memory, loader, image, &plan);
    }
    let invalidation = memory.stats().pending_invalidation.ok_or_else(|| {
        LoaderError::runtime_defect(
            "import-binding",
            "materialized-image-delta-missing",
            "unresolved import planning lost the materialized image memory generation",
        )
    })?;
    let first_unresolved_import = plan.unresolved_imports.first().cloned();
    Ok(ImportBinding {
        source: plan.source,
        image_owner: plan.image_owner,
        image_load_base: plan.image_load_base,
        total_imports: plan.total_imports,
        bound_imports: 0,
        unsupported_trap_imports: 0,
        remaining_imports: plan.total_imports,
        complete: false,
        image_owner_sealed: false,
        arena: None,
        thunks: Vec::new(),
        unresolved_imports: plan.unresolved_imports,
        first_unresolved_import,
        invalidation,
        stop: UnsupportedPoint {
            stage: "import-resolution",
            code: "import-symbols-unimplemented",
            message: "one or more planned imports have no declared runtime endpoint; all unresolved identities were reported and guest memory was left unchanged",
        },
    })
}

pub fn commit_pe32_import_binding(
    memory: &mut GuestMemory,
    loader: &LoaderIngressReport,
    image: &ImageMaterialization,
    plan: &ImportBindingPlan,
) -> Result<ImportBinding, LoaderError> {
    let mut checkpoint = |_| Ok(());
    commit_pe32_import_binding_with_checkpoint(memory, loader, image, plan, &mut checkpoint)
}

fn commit_pe32_import_binding_with_checkpoint<F>(
    memory: &mut GuestMemory,
    loader: &LoaderIngressReport,
    image: &ImageMaterialization,
    plan: &ImportBindingPlan,
    checkpoint: &mut F,
) -> Result<ImportBinding, LoaderError>
where
    F: FnMut(ImportCommitStage) -> Result<(), LoaderError>,
{
    validate_import_mapping_allocation(&plan.address_allocation)?;
    validate_binding_input(memory, loader, image)?;
    if !plan.complete || !plan.unresolved_imports.is_empty() {
        return Err(LoaderError::invalid_input(
            "import-binding",
            "incomplete-import-plan-cannot-commit",
            "an import plan with unresolved imports cannot mutate guest memory",
        ));
    }
    if plan.source != loader.source
        || plan.image_owner != image.layout.owner
        || plan.image_load_base != image.load_base
        || plan.total_imports != plan.imports.len()
    {
        return Err(LoaderError::invalid_input(
            "import-binding",
            "import-plan-image-mismatch",
            "import plan identity or import count does not match the materialized image",
        ));
    }
    if memory.stats().generation != plan.memory_generation {
        return Err(LoaderError::invalid_input(
            "import-binding",
            "import-plan-memory-generation-stale",
            "guest memory changed after import planning; build a new plan before commit",
        ));
    }
    for planned in &plan.imports {
        let current = read_initialization_u32(memory, image.layout.owner, planned.iat_address)?;
        if current != planned.original_iat_value {
            return Err(LoaderError::invalid_input(
                "import-binding",
                "iat-slot-changed-after-planning",
                format!(
                    "IAT slot {:#010x} changed after planning and before atomic commit",
                    planned.iat_address.0
                ),
            ));
        }
    }
    if !plan.imports.is_empty() {
        memory
            .preflight_growth(1, 1, plan.required_pages, "import-binding")
            .map_err(memory_error)?;
        memory
            .preflight_fixed_reservation(
                IMPORT_THUNK_ARENA_BASE,
                IMPORT_THUNK_ARENA_SIZE,
                "import-binding",
            )
            .map_err(memory_error)?;
    }

    let mut transaction = memory.begin_transaction();
    let mut thunks = Vec::with_capacity(plan.imports.len());
    let arena = if plan.imports.is_empty() {
        None
    } else {
        let arena_owner = transaction
            .create_owner(MemoryOwnerKind::Runtime)
            .map_err(memory_error)?;
        checkpoint(ImportCommitStage::ArenaOwnerCreated)?;
        let arena_range = transaction
            .reserve_fixed_for_address_allocation(
                plan.address_allocation.id.0,
                arena_owner,
                MemoryRegionKind::Runtime,
                IMPORT_THUNK_ARENA_BASE,
                IMPORT_THUNK_ARENA_SIZE,
            )
            .map_err(memory_error)?;
        checkpoint(ImportCommitStage::ArenaReserved)?;
        let committed_range = GuestRange::new(
            IMPORT_THUNK_ARENA_BASE,
            u64::try_from(plan.committed_bytes).map_err(|_| {
                LoaderError::resource_limit(
                    "import-binding",
                    "thunk-commit-size-not-representable",
                    "thunk commit size does not fit 64-bit guest range accounting",
                )
            })?,
        )
        .map_err(memory_error)?;
        transaction
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
        checkpoint(ImportCommitStage::ArenaCommitted)?;

        fill_thunk_pages(&mut transaction, arena_owner, plan.committed_bytes)?;
        checkpoint(ImportCommitStage::ThunkPagesFilled)?;
        for (index, planned) in plan.imports.iter().enumerate() {
            let disposition = match planned.disposition {
                ImportPlanDisposition::DeclaredRuntimeExport => {
                    ImportThunkDisposition::DeclaredRuntimeExport
                }
                ImportPlanDisposition::UnsupportedTrap => ImportThunkDisposition::UnsupportedTrap,
                ImportPlanDisposition::Unresolved => {
                    return Err(LoaderError::runtime_defect(
                        "import-binding",
                        "complete-import-plan-contains-unresolved-entry",
                        "a complete import plan contains an unresolved disposition",
                    ));
                }
            };
            transaction
                .write_initialization(arena_owner, planned.thunk_address, &[0x0f, 0x0b])
                .map_err(memory_error)?;
            checkpoint(ImportCommitStage::ThunkWritten(index))?;
            transaction
                .write_initialization(
                    image.layout.owner,
                    planned.iat_address,
                    &planned.thunk_address.0.to_le_bytes(),
                )
                .map_err(memory_error)?;
            checkpoint(ImportCommitStage::IatWritten(index))?;
            thunks.push(ImportThunk {
                id: planned.id,
                address: planned.thunk_address,
                requester_owner: planned.requester_owner,
                module: planned.module.clone(),
                iat_rva: planned.iat_rva,
                iat_address: planned.iat_address,
                original_iat_value: planned.original_iat_value,
                disposition,
                target: planned.target.clone(),
            });
        }
        verify_bound_iat(&transaction, image.layout.owner, &thunks)?;
        checkpoint(ImportCommitStage::IatVerified)?;
        let content_sha256 = hash_initialization_range(
            &transaction,
            arena_owner,
            IMPORT_THUNK_ARENA_BASE,
            plan.committed_bytes,
        )?;
        transaction.seal_owner(arena_owner).map_err(memory_error)?;
        checkpoint(ImportCommitStage::ArenaSealed)?;
        Some(ImportThunkArena {
            owner: arena_owner,
            range: arena_range,
            committed_pages: plan.required_pages,
            slot_size: IMPORT_THUNK_SLOT_SIZE,
            slot_capacity: IMPORT_THUNK_CAPACITY,
            bound_slots: thunks.len(),
            content_sha256,
            owner_sealed: true,
        })
    };

    transaction
        .seal_owner(image.layout.owner)
        .map_err(memory_error)?;
    checkpoint(ImportCommitStage::ImageSealed)?;
    let pending = transaction.stats().pending_invalidation.ok_or_else(|| {
        LoaderError::runtime_defect(
            "import-binding",
            "binding-invalidation-missing",
            "import binding completed without a pending memory invalidation",
        )
    })?;
    transaction.commit_transaction();
    let bound_imports = thunks.len();
    let unsupported_trap_imports = thunks
        .iter()
        .filter(|thunk| thunk.disposition == ImportThunkDisposition::UnsupportedTrap)
        .count();
    let remaining_imports = plan
        .total_imports
        .checked_sub(bound_imports)
        .ok_or_else(import_binding_accounting_overflow)?;
    let stop = if unsupported_trap_imports != 0 {
        UnsupportedPoint {
            stage: "thunk-dispatch",
            code: "unsupported-import-trap-dispatch-pending",
            message: "all planned imports are bound and the image is sealed; catalog-missing imports point to identity-preserving UD2 traps that return no value, and guest execution has not been attempted",
        }
    } else {
        UnsupportedPoint {
            stage: "process-bootstrap",
            code: "peb-teb-process-parameters-unimplemented",
            message: "all planned imports are bound to runtime-owned thunks and the image is sealed; PEB/TEB/process parameters, initial stack, thunk dispatch, and guest execution were not attempted",
        }
    };

    Ok(ImportBinding {
        source: plan.source.clone(),
        image_owner: plan.image_owner,
        image_load_base: plan.image_load_base,
        total_imports: plan.total_imports,
        bound_imports,
        unsupported_trap_imports,
        remaining_imports,
        complete: true,
        image_owner_sealed: true,
        arena,
        thunks,
        unresolved_imports: Vec::new(),
        first_unresolved_import: None,
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
    if image.layout.owner_sealed
        || memory
            .owner_is_sealed(image.layout.owner)
            .map_err(memory_error)?
    {
        return Err(LoaderError::invalid_input(
            "import-binding",
            "imports-already-bound",
            "the materialized image owner is sealed; successful import binding cannot be repeated",
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

fn validate_import_mapping_allocation(
    allocation: &ProcessAddressAllocationRecord,
) -> Result<(), LoaderError> {
    validate_process_address_mapping_allocation_for_memory(
        allocation,
        &ProcessAddressMappingRequirements {
            purpose: ProcessAddressAllocationPurpose::ImportThunks,
            owner: "runtime-import-thunks".to_owned(),
            range: RuntimeAddressRange {
                base: IMPORT_THUNK_ARENA_BASE,
                byte_length: IMPORT_THUNK_ARENA_SIZE,
            },
            classification: ProcessAddressAllocationClassification::RuntimeControl,
            lifetime: ProcessAddressAllocationLifetime::Process,
            release_policy: ProcessAddressAllocationReleasePolicy::Permanent,
            executable: true,
        },
    )
    .map_err(memory_error)
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::{
        MemoryLimits, ModuleSnapshot, Pe32Limits, ProcessAddressSpaceAllocator,
        ProcessAddressSpaceLayout, build_pe32_load_plan, materialize_pe32_image,
    };

    const LOAD_BASE: GuestAddress = GuestAddress(0x1000_0000);

    #[test]
    fn rolls_back_each_import_commit_checkpoint_and_allows_retry() {
        let stages = [
            ImportCommitStage::ArenaOwnerCreated,
            ImportCommitStage::ArenaReserved,
            ImportCommitStage::ArenaCommitted,
            ImportCommitStage::ThunkPagesFilled,
            ImportCommitStage::ThunkWritten(0),
            ImportCommitStage::IatWritten(0),
            ImportCommitStage::ThunkWritten(1),
            ImportCommitStage::IatWritten(1),
            ImportCommitStage::IatVerified,
            ImportCommitStage::ArenaSealed,
            ImportCommitStage::ImageSealed,
        ];

        for fail_at in stages {
            let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(".work/toolchain-spike/abi-smoke.exe");
            let source = fs::read(&fixture).unwrap_or_else(|error| {
                panic!(
                    "cannot read {} ({error}); run `npm run check:toolchain` first",
                    fixture.display()
                )
            });
            let limits = Pe32Limits::default();
            let snapshot = ModuleSnapshot::from_bytes(source, &limits).expect("snapshot");
            let loader = build_pe32_load_plan(&snapshot, limits).expect("load plan");
            let mut memory = GuestMemory::new(MemoryLimits {
                max_physical_pages: 6,
                max_regions: 2,
                max_owners: 2,
                max_access_bytes: GUEST_PAGE_SIZE as usize,
            })
            .expect("memory");
            let image = materialize_pe32_image(&mut memory, &snapshot, &loader, LOAD_BASE)
                .expect("materialized image");
            let catalog = RuntimeExportCatalog::new(vec![
                RuntimeExport::named("vine_fixture.dll", "fixture_add"),
                RuntimeExport::named("vine_fixture.dll", "fixture_call_callback"),
            ])
            .expect("catalog");
            let allocator =
                ProcessAddressSpaceAllocator::vine_v1(ProcessAddressSpaceLayout::vine_v1())
                    .expect("address allocator");
            let allocation = allocator
                .allocation_for_purpose(&ProcessAddressAllocationPurpose::ImportThunks)
                .expect("import allocation");
            let plan =
                plan_pe32_imports(&memory, allocation, &loader, &image, &catalog).expect("plan");
            let before = memory.stats();
            let original_iat = plan
                .imports
                .iter()
                .map(|planned| {
                    read_initialization_u32(&memory, image.layout.owner, planned.iat_address)
                        .expect("original IAT")
                })
                .collect::<Vec<_>>();
            let mut checkpoint = |stage| {
                if stage == fail_at {
                    Err(LoaderError::invalid_input(
                        "import-binding-test",
                        "injected-import-commit-failure",
                        format!("injected failure after {stage:?}"),
                    ))
                } else {
                    Ok(())
                }
            };
            let error = commit_pe32_import_binding_with_checkpoint(
                &mut memory,
                &loader,
                &image,
                &plan,
                &mut checkpoint,
            )
            .expect_err("injected checkpoint must fail");
            assert_eq!(error.code, "injected-import-commit-failure");
            assert_eq!(memory.stats(), before, "rollback after {fail_at:?}");
            assert!(
                !memory
                    .owner_is_sealed(image.layout.owner)
                    .expect("image owner")
            );
            for (planned, original) in plan.imports.iter().zip(original_iat) {
                assert_eq!(
                    read_initialization_u32(&memory, image.layout.owner, planned.iat_address,)
                        .expect("rolled-back IAT"),
                    original,
                    "IAT rollback after {fail_at:?}"
                );
            }
            commit_pe32_import_binding(&mut memory, &loader, &image, &plan)
                .unwrap_or_else(|error| panic!("retry after {fail_at:?} failed: {error}"));
        }
    }
}
