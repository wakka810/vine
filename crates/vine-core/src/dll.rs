use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use crate::imports::{
    IMPORT_THUNK_ARENA_BASE, IMPORT_THUNK_ARENA_SIZE, IMPORT_THUNK_SLOT_SIZE, ImportThunkArena,
    ImportThunkId, RuntimeExportCatalog,
};
use crate::materialize::ImageMaterialization;
use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestRange, MemoryOwnerKind, MemoryRegionKind,
    PageProtection,
};
use crate::module_graph::RuntimeModuleClass;
use crate::module_images::{RuntimeModuleImageError, RuntimeModuleImageErrorClass};
use crate::pe32::{ExportLookup, ImageKind, ImportTarget, LoaderIngressReport};
use crate::runtime::{
    RuntimeGuestImport, RuntimeGuestImportBinding, RuntimeGuestImportResolution,
    RuntimeModuleDependencyEdge, RuntimeState, RuntimeStateError, RuntimeSystemExportEndpoint,
    RuntimeSystemExportTable,
};

const IMPORT_THUNK_CAPACITY: usize =
    (IMPORT_THUNK_ARENA_SIZE / IMPORT_THUNK_SLOT_SIZE as u64) as usize;

pub(crate) fn bind_guest_module_imports(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    requester_handle: u32,
    loader: &LoaderIngressReport,
    image: &ImageMaterialization,
    runtime_catalog: &RuntimeExportCatalog,
) -> Result<RuntimeGuestImportBinding, RuntimeStateError> {
    validate_requester(memory, runtime, requester_handle, loader, image)?;
    let requester_id = runtime
        .module_graph()
        .module_by_handle(requester_handle)
        .map_err(runtime_module_graph_error)?
        .id;
    let memory_generation = memory.stats().generation;
    let mut imports = Vec::new();
    let mut dependency_edges = BTreeSet::new();
    let mut iat_addresses = BTreeSet::new();
    let mut needs_runtime_export_table = false;

    for module in &loader.load_plan.imports {
        let requested_node = runtime
            .module_graph()
            .module_by_name(&module.name)
            .map_err(runtime_module_graph_error)?;
        dependency_edges.insert(RuntimeModuleDependencyEdge {
            requester: requester_id,
            dependency: requested_node.id,
        });
        for symbol in &module.symbols {
            let iat_address = add_rva(image.load_base, symbol.iat_rva.0)?;
            if !iat_addresses.insert(iat_address) {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-guest-imports",
                    "guest-import-iat-duplicate",
                    "guest import plan contains the same IAT address more than once",
                ));
            }
            let current = read_initialization_u32(memory, image.layout.owner, iat_address)?;
            if current != symbol.original_iat_value {
                return Err(RuntimeStateError::conflict(
                    "runtime-guest-imports",
                    "guest-import-iat-not-original",
                    format!(
                        "IAT address {:#010x} no longer contains its immutable planned value",
                        iat_address.0
                    ),
                ));
            }
            let resolution = if requested_node.class == RuntimeModuleClass::RuntimeSystemModule {
                needs_runtime_export_table = true;
                let endpoint = planned_runtime_export_endpoint(
                    runtime,
                    runtime_catalog,
                    &module.name,
                    &symbol.target,
                )?;
                RuntimeGuestImportResolution::RuntimeExport { endpoint }
            } else {
                let requested_image = runtime
                    .module_images()
                    .image_by_name(&module.name)
                    .map_err(runtime_module_image_error)?;
                if requested_image.module_id != requested_node.id {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-guest-imports",
                        "guest-import-module-image-identity-mismatch",
                        "module graph and mapped image registry disagree on the requested module",
                    ));
                }
                let lookup = match &symbol.target {
                    ImportTarget::Name { name, .. } => ExportLookup::Name { name: name.clone() },
                    ImportTarget::Ordinal { ordinal } => ExportLookup::Ordinal {
                        ordinal: u32::from(*ordinal),
                    },
                };
                let export = runtime
                    .resolve_guest_export(&module.name, lookup)
                    .map_err(runtime_module_image_error)?;
                for hop in &export.forwarders {
                    let target = runtime
                        .module_images()
                        .image_by_name(&hop.target_module)
                        .map_err(runtime_module_image_error)?;
                    dependency_edges.insert(RuntimeModuleDependencyEdge {
                        requester: hop.source_module_id,
                        dependency: target.module_id,
                    });
                }
                RuntimeGuestImportResolution::GuestExport { export }
            };
            imports.push(RuntimeGuestImport {
                requested_module_id: requested_node.id,
                requested_module: requested_node.canonical_name.clone(),
                target: symbol.target.clone(),
                iat_address,
                original_iat_value: symbol.original_iat_value,
                resolution,
            });
        }
    }

    let plan = runtime.plan_guest_import_binding(
        requester_handle,
        loader.source.clone(),
        image.layout.owner,
        image.load_base,
        imports,
        dependency_edges.into_iter().collect(),
    )?;
    if memory.stats().generation != memory_generation {
        return Err(RuntimeStateError::conflict(
            "runtime-guest-imports",
            "guest-import-memory-generation-stale",
            "guest memory changed during import planning",
        ));
    }
    let install_runtime_export_table =
        needs_runtime_export_table && runtime.runtime_system_exports().is_none();
    if install_runtime_export_table {
        preflight_runtime_export_table(memory, runtime_catalog)?;
    }

    let mut transaction = memory.begin_transaction();
    let runtime_system_exports = if install_runtime_export_table {
        Some(install_runtime_export_table_in_memory(
            &mut transaction,
            runtime_catalog,
        )?)
    } else {
        None
    };
    for import in &plan.imports {
        let current =
            read_initialization_u32(&transaction, image.layout.owner, import.iat_address)?;
        if current != import.original_iat_value {
            return Err(RuntimeStateError::conflict(
                "runtime-guest-imports",
                "guest-import-iat-changed-before-commit",
                "IAT value changed after guest import planning",
            ));
        }
        transaction
            .write_initialization(
                image.layout.owner,
                import.iat_address,
                &import.resolved_address().0.to_le_bytes(),
            )
            .map_err(RuntimeStateError::from_memory)?;
    }
    for import in &plan.imports {
        let bound = read_initialization_u32(&transaction, image.layout.owner, import.iat_address)?;
        if bound != import.resolved_address().0 {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-guest-imports",
                "guest-import-iat-verification-failed",
                "guest import IAT readback differs from the resolved export address",
            ));
        }
    }
    transaction
        .seal_owner(image.layout.owner)
        .map_err(RuntimeStateError::from_memory)?;
    let invalidation = transaction.stats().pending_invalidation.ok_or_else(|| {
        RuntimeStateError::runtime_defect(
            "runtime-guest-imports",
            "guest-import-invalidation-missing",
            "guest import binding completed without a pending memory invalidation",
        )
    })?;
    let binding = runtime
        .commit_guest_import_binding(plan, invalidation, runtime_system_exports)?
        .clone();
    transaction.commit_transaction();
    Ok(binding)
}

fn planned_runtime_export_endpoint(
    runtime: &RuntimeState,
    catalog: &RuntimeExportCatalog,
    module: &str,
    target: &ImportTarget,
) -> Result<RuntimeSystemExportEndpoint, RuntimeStateError> {
    let (index, export) = catalog.resolve_with_index(module, target).ok_or_else(|| {
        RuntimeStateError::not_found(
            "runtime-guest-imports",
            "runtime-export-catalog-symbol-unknown",
            format!("runtime export catalog does not declare {module}!{target:?}"),
        )
    })?;
    let endpoint = runtime_export_endpoint(index, export.clone())?;
    if let Some(table) = runtime.runtime_system_exports() {
        validate_runtime_export_table_catalog(table, catalog)?;
        let installed = table.endpoints.get(index).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-guest-imports",
                "runtime-export-endpoint-index-missing",
                "installed runtime export table is missing a catalog endpoint",
            )
        })?;
        if installed != &endpoint {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-guest-imports",
                "runtime-export-endpoint-identity-mismatch",
                "installed runtime export endpoint disagrees with its ordered catalog identity",
            ));
        }
    }
    Ok(endpoint)
}

fn validate_runtime_export_table_catalog(
    table: &RuntimeSystemExportTable,
    catalog: &RuntimeExportCatalog,
) -> Result<(), RuntimeStateError> {
    if table.endpoints.len() != catalog.len() {
        return Err(RuntimeStateError::conflict(
            "runtime-guest-imports",
            "runtime-export-catalog-changed",
            "runtime export catalog changed after its process thunk table was installed",
        ));
    }
    for (index, export) in catalog.exports().iter().enumerate() {
        if table.endpoints.get(index) != Some(&runtime_export_endpoint(index, export.clone())?) {
            return Err(RuntimeStateError::conflict(
                "runtime-guest-imports",
                "runtime-export-catalog-changed",
                "runtime export catalog order or identity changed after installation",
            ));
        }
    }
    Ok(())
}

fn runtime_export_endpoint(
    index: usize,
    export: crate::imports::RuntimeExport,
) -> Result<RuntimeSystemExportEndpoint, RuntimeStateError> {
    if index >= IMPORT_THUNK_CAPACITY {
        return Err(RuntimeStateError::resource_limit(
            "runtime-guest-imports",
            "runtime-export-thunk-capacity-exceeded",
            "runtime export catalog exceeds the process import-thunk arena capacity",
        ));
    }
    let id = ImportThunkId(u32::try_from(index + 1).map_err(|_| {
        RuntimeStateError::resource_limit(
            "runtime-guest-imports",
            "runtime-export-thunk-id-overflow",
            "runtime export endpoint ID does not fit 32 bits",
        )
    })?);
    let offset = u32::try_from(index)
        .ok()
        .and_then(|index| index.checked_mul(IMPORT_THUNK_SLOT_SIZE))
        .ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-guest-imports",
                "runtime-export-thunk-offset-overflow",
                "runtime export endpoint offset overflowed 32-bit accounting",
            )
        })?;
    let address = IMPORT_THUNK_ARENA_BASE
        .0
        .checked_add(offset)
        .map(GuestAddress)
        .ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-guest-imports",
                "runtime-export-thunk-address-overflow",
                "runtime export endpoint address overflowed x86 accounting",
            )
        })?;
    Ok(RuntimeSystemExportEndpoint {
        id,
        address,
        export,
    })
}

fn preflight_runtime_export_table(
    memory: &GuestMemory,
    catalog: &RuntimeExportCatalog,
) -> Result<(), RuntimeStateError> {
    let committed_bytes = runtime_export_committed_bytes(catalog)?;
    let committed_pages = committed_bytes / GUEST_PAGE_SIZE as usize;
    memory
        .preflight_growth(1, 1, committed_pages, "runtime-guest-imports")
        .map_err(RuntimeStateError::from_memory)?;
    memory
        .preflight_fixed_reservation(
            IMPORT_THUNK_ARENA_BASE,
            IMPORT_THUNK_ARENA_SIZE,
            "runtime-guest-imports",
        )
        .map(|_| ())
        .map_err(RuntimeStateError::from_memory)
}

fn install_runtime_export_table_in_memory(
    memory: &mut GuestMemory,
    catalog: &RuntimeExportCatalog,
) -> Result<RuntimeSystemExportTable, RuntimeStateError> {
    let committed_bytes = runtime_export_committed_bytes(catalog)?;
    let committed_pages = committed_bytes / GUEST_PAGE_SIZE as usize;
    let owner = memory
        .create_owner(MemoryOwnerKind::Runtime)
        .map_err(RuntimeStateError::from_memory)?;
    let range = memory
        .reserve_fixed(
            owner,
            MemoryRegionKind::Runtime,
            IMPORT_THUNK_ARENA_BASE,
            IMPORT_THUNK_ARENA_SIZE,
        )
        .map_err(RuntimeStateError::from_memory)?;
    let committed_range = GuestRange::new(
        IMPORT_THUNK_ARENA_BASE,
        u64::try_from(committed_bytes).map_err(|_| {
            RuntimeStateError::resource_limit(
                "runtime-guest-imports",
                "runtime-export-commit-size-overflow",
                "runtime export table commit size does not fit 64-bit accounting",
            )
        })?,
    )
    .map_err(RuntimeStateError::from_memory)?;
    memory
        .commit(
            owner,
            committed_range,
            PageProtection {
                read: true,
                write: false,
                execute: true,
            },
        )
        .map_err(RuntimeStateError::from_memory)?;
    fill_runtime_export_table(memory, owner, committed_bytes)?;
    let mut endpoints = Vec::with_capacity(catalog.len());
    for (index, export) in catalog.exports().iter().cloned().enumerate() {
        let endpoint = runtime_export_endpoint(index, export)?;
        memory
            .write_initialization(owner, endpoint.address, &[0x0f, 0x0b])
            .map_err(RuntimeStateError::from_memory)?;
        endpoints.push(endpoint);
    }
    let content_sha256 =
        hash_initialization_range(memory, owner, IMPORT_THUNK_ARENA_BASE, committed_bytes)?;
    memory
        .seal_owner(owner)
        .map_err(RuntimeStateError::from_memory)?;
    Ok(RuntimeSystemExportTable {
        arena: ImportThunkArena {
            owner,
            range,
            committed_pages,
            slot_size: IMPORT_THUNK_SLOT_SIZE,
            slot_capacity: IMPORT_THUNK_CAPACITY,
            bound_slots: endpoints.len(),
            content_sha256,
            owner_sealed: true,
        },
        endpoints,
    })
}

fn runtime_export_committed_bytes(
    catalog: &RuntimeExportCatalog,
) -> Result<usize, RuntimeStateError> {
    if catalog.is_empty() {
        return Err(RuntimeStateError::invalid_input(
            "runtime-guest-imports",
            "runtime-export-catalog-empty",
            "a runtime-system import requires at least one declared runtime export",
        ));
    }
    if catalog.len() > IMPORT_THUNK_CAPACITY {
        return Err(RuntimeStateError::resource_limit(
            "runtime-guest-imports",
            "runtime-export-thunk-capacity-exceeded",
            "runtime export catalog exceeds the process import-thunk arena capacity",
        ));
    }
    let used = catalog
        .len()
        .checked_mul(IMPORT_THUNK_SLOT_SIZE as usize)
        .ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-guest-imports",
                "runtime-export-table-size-overflow",
                "runtime export table size overflowed host accounting",
            )
        })?;
    let page_size = GUEST_PAGE_SIZE as usize;
    used.checked_add(page_size - 1)
        .map(|adjusted| adjusted / page_size * page_size)
        .ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-guest-imports",
                "runtime-export-table-size-overflow",
                "runtime export table alignment overflowed host accounting",
            )
        })
}

fn fill_runtime_export_table(
    memory: &mut GuestMemory,
    owner: crate::memory::MemoryOwnerId,
    byte_length: usize,
) -> Result<(), RuntimeStateError> {
    let chunk_limit = memory.limits().max_access_bytes;
    let halt_bytes = vec![0xf4; chunk_limit.min(byte_length)];
    let mut offset = 0usize;
    while offset < byte_length {
        let chunk_length = (byte_length - offset).min(halt_bytes.len());
        let offset_u32 = u32::try_from(offset).map_err(|_| {
            RuntimeStateError::resource_limit(
                "runtime-guest-imports",
                "runtime-export-fill-offset-overflow",
                "runtime export table fill offset does not fit 32 bits",
            )
        })?;
        let address = IMPORT_THUNK_ARENA_BASE
            .0
            .checked_add(offset_u32)
            .map(GuestAddress)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-guest-imports",
                    "runtime-export-fill-address-overflow",
                    "runtime export table fill address overflowed x86 accounting",
                )
            })?;
        memory
            .write_initialization(owner, address, &halt_bytes[..chunk_length])
            .map_err(RuntimeStateError::from_memory)?;
        offset = offset.checked_add(chunk_length).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-guest-imports",
                "runtime-export-fill-offset-overflow",
                "runtime export table fill offset overflowed host accounting",
            )
        })?;
    }
    Ok(())
}

fn hash_initialization_range(
    memory: &GuestMemory,
    owner: crate::memory::MemoryOwnerId,
    address: GuestAddress,
    byte_length: usize,
) -> Result<String, RuntimeStateError> {
    let chunk_limit = memory.limits().max_access_bytes;
    let mut hasher = Sha256::new();
    let mut offset = 0usize;
    while offset < byte_length {
        let chunk_length = (byte_length - offset).min(chunk_limit);
        let offset_u32 = u32::try_from(offset).map_err(|_| {
            RuntimeStateError::resource_limit(
                "runtime-guest-imports",
                "runtime-export-hash-offset-overflow",
                "runtime export table hash offset does not fit 32 bits",
            )
        })?;
        let chunk_address = address
            .0
            .checked_add(offset_u32)
            .map(GuestAddress)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-guest-imports",
                    "runtime-export-hash-address-overflow",
                    "runtime export table hash address overflowed x86 accounting",
                )
            })?;
        hasher.update(
            memory
                .read_initialization(owner, chunk_address, chunk_length)
                .map_err(RuntimeStateError::from_memory)?,
        );
        offset = offset.checked_add(chunk_length).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-guest-imports",
                "runtime-export-hash-offset-overflow",
                "runtime export table hash offset overflowed host accounting",
            )
        })?;
    }
    Ok(hex_digest(&hasher.finalize()))
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

fn validate_requester(
    memory: &GuestMemory,
    runtime: &RuntimeState,
    requester_handle: u32,
    loader: &LoaderIngressReport,
    image: &ImageMaterialization,
) -> Result<(), RuntimeStateError> {
    runtime.module(requester_handle)?;
    let node = runtime
        .module_graph()
        .module_by_handle(requester_handle)
        .map_err(runtime_module_graph_error)?;
    let expected_kind = match node.class {
        RuntimeModuleClass::MainExecutable => ImageKind::Executable,
        RuntimeModuleClass::GuestDynamicLibrary => ImageKind::DynamicLibrary,
        RuntimeModuleClass::RuntimeSystemModule => {
            return Err(RuntimeStateError::invalid_input(
                "runtime-guest-imports",
                "runtime-system-module-guest-binding-forbidden",
                "runtime system modules do not own guest PE import tables",
            ));
        }
    };
    let expected_entry_point = image
        .load_base
        .0
        .checked_add(loader.load_plan.image.entry_point_rva.0)
        .map(GuestAddress)
        .ok_or_else(|| {
            RuntimeStateError::invalid_input(
                "runtime-guest-imports",
                "guest-import-entry-point-overflow",
                "requester load base plus entry-point RVA overflowed",
            )
        })?;
    if loader.schema != "vine.pe32-loader-ingress"
        || loader.schema_version != 1
        || loader.source != image.source
        || image.layout.source != image.source
        || loader.load_plan.image.kind != expected_kind
        || image.preferred_image_base.0 != loader.load_plan.image.preferred_image_base
        || image.load_base != image.layout.image_range.start()
        || image.layout.image_range.byte_length() != u64::from(loader.load_plan.image.size_of_image)
        || image.relocation_delta
            != image
                .load_base
                .0
                .wrapping_sub(loader.load_plan.image.preferred_image_base)
        || image.entry_point_address != expected_entry_point
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-guest-imports",
            "guest-import-requester-identity-mismatch",
            "requester loader, materialization, graph class, or address identity disagrees",
        ));
    }
    memory
        .validate_unsealed_image_owner(image.layout.owner, &image.source, image.layout.image_range)
        .map_err(RuntimeStateError::from_memory)
}

fn add_rva(load_base: GuestAddress, rva: u32) -> Result<GuestAddress, RuntimeStateError> {
    load_base
        .0
        .checked_add(rva)
        .map(GuestAddress)
        .ok_or_else(|| {
            RuntimeStateError::invalid_input(
                "runtime-guest-imports",
                "guest-import-iat-address-overflow",
                "requester load base plus IAT RVA overflowed",
            )
        })
}

fn read_initialization_u32(
    memory: &GuestMemory,
    owner: crate::memory::MemoryOwnerId,
    address: GuestAddress,
) -> Result<u32, RuntimeStateError> {
    let bytes = memory
        .read_initialization(owner, address, 4)
        .map_err(RuntimeStateError::from_memory)?;
    let bytes: [u8; 4] = bytes.try_into().map_err(|_| {
        RuntimeStateError::runtime_defect(
            "runtime-guest-imports",
            "guest-import-iat-width-invalid",
            "preflighted IAT read did not return four bytes",
        )
    })?;
    Ok(u32::from_le_bytes(bytes))
}

fn runtime_module_graph_error(
    error: crate::module_graph::RuntimeModuleGraphError,
) -> RuntimeStateError {
    use crate::module_graph::RuntimeModuleGraphErrorClass;
    match error.class {
        RuntimeModuleGraphErrorClass::InvalidInput => {
            RuntimeStateError::invalid_input("runtime-module-graph", error.code, error.message)
        }
        RuntimeModuleGraphErrorClass::NotFound => {
            RuntimeStateError::not_found("runtime-module-graph", error.code, error.message)
        }
        RuntimeModuleGraphErrorClass::Conflict => {
            RuntimeStateError::conflict("runtime-module-graph", error.code, error.message)
        }
        RuntimeModuleGraphErrorClass::ResourceLimit => {
            RuntimeStateError::resource_limit("runtime-module-graph", error.code, error.message)
        }
        RuntimeModuleGraphErrorClass::RuntimeDefect => {
            RuntimeStateError::runtime_defect("runtime-module-graph", error.code, error.message)
        }
    }
}

fn runtime_module_image_error(error: RuntimeModuleImageError) -> RuntimeStateError {
    match error.class {
        RuntimeModuleImageErrorClass::InvalidInput => {
            RuntimeStateError::invalid_input("runtime-module-image", error.code, error.message)
        }
        RuntimeModuleImageErrorClass::NotFound => {
            RuntimeStateError::not_found("runtime-module-image", error.code, error.message)
        }
        RuntimeModuleImageErrorClass::Conflict => {
            RuntimeStateError::conflict("runtime-module-image", error.code, error.message)
        }
        RuntimeModuleImageErrorClass::ResourceLimit => {
            RuntimeStateError::resource_limit("runtime-module-image", error.code, error.message)
        }
        RuntimeModuleImageErrorClass::RuntimeDefect => {
            RuntimeStateError::runtime_defect("runtime-module-image", error.code, error.message)
        }
    }
}
