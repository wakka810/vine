use std::collections::BTreeSet;

use crate::imports::{
    IMPORT_THUNK_ARENA_BASE, IMPORT_THUNK_ARENA_SIZE, IMPORT_THUNK_SLOT_SIZE, ImportThunkId,
    RuntimeExportCatalog,
};
use crate::materialize::ImageMaterialization;
use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestRange, MemoryOwnerKind, MemoryRegionKind,
    PageProtection,
};
use crate::module_graph::RuntimeModuleClass;
use crate::module_images::RuntimeModuleImageError;
use crate::pe32::{ExportLookup, ImageKind, ImportTarget, Pe32LoadPlan};
use crate::runtime::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationPurpose,
    ProcessAddressAllocationRecord, ProcessAddressAllocationReleasePolicy,
    ProcessAddressMappingRequirements, Runtime, RuntimeAddressRange, RuntimeGuestImport,
    RuntimeGuestImportBinding, RuntimeGuestImportResolution, RuntimeModuleDependencyEdge,
    RuntimeStateError, RuntimeSystemExportEndpoint, RuntimeSystemExportTable,
    validate_process_address_mapping_allocation,
};

const IMPORT_THUNK_CAPACITY: usize =
    (IMPORT_THUNK_ARENA_SIZE / IMPORT_THUNK_SLOT_SIZE as u64) as usize;

pub(crate) fn bind_guest_module_imports(
    runtime: &mut Runtime,
    requester_handle: u32,
    loader: &Pe32LoadPlan,
    image: &ImageMaterialization,
    runtime_catalog: &RuntimeExportCatalog,
) -> Result<RuntimeGuestImportBinding, RuntimeStateError> {
    let mut transaction = runtime.memory.begin_transaction();
    let binding = bind_guest_module_imports_transaction(
        &mut transaction,
        &mut runtime.modules,
        &runtime.address_space_allocator,
        requester_handle,
        loader,
        image,
        runtime_catalog,
    )?;
    transaction.commit_transaction();
    Ok(binding)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn bind_guest_module_imports_transaction(
    memory: &mut GuestMemory,
    modules: &mut crate::runtime::RuntimeModules,
    address_space: &crate::runtime::ProcessAddressSpaceAllocator,
    requester_handle: u32,
    loader: &Pe32LoadPlan,
    image: &ImageMaterialization,
    runtime_catalog: &RuntimeExportCatalog,
) -> Result<RuntimeGuestImportBinding, RuntimeStateError> {
    validate_requester(memory, modules, requester_handle, loader, image)?;
    let requester_id = modules
        .graph()
        .module_by_handle(requester_handle)
        .map_err(runtime_module_graph_error)?
        .id;
    let mut imports = Vec::new();
    let mut dependency_edges = BTreeSet::new();
    let mut iat_addresses = BTreeSet::new();
    let mut needs_runtime_export_table = false;

    for module in &loader.imports {
        let requested_node = modules
            .graph()
            .module_by_name(&module.name)
            .map_err(runtime_module_graph_error)?;
        dependency_edges.insert(RuntimeModuleDependencyEdge {
            requester: requester_id,
            dependency: requested_node.id,
        });
        for symbol in &module.symbols {
            let iat_address = add_rva(image.layout.image_range.start(), symbol.iat_rva.0)?;
            if !iat_addresses.insert(iat_address) {
                return Err(RuntimeStateError::new(
                    "guest import plan contains the same IAT address more than once",
                ));
            }
            let current = read_initialization_u32(memory, image.layout.owner, iat_address)?;
            if current != symbol.original_iat_value {
                return Err(RuntimeStateError::new(format!(
                    "IAT address {:#010x} no longer contains its immutable planned value",
                    iat_address.0
                )));
            }
            let resolution = if requested_node.class == RuntimeModuleClass::RuntimeSystemModule {
                needs_runtime_export_table = true;
                let endpoint = planned_runtime_export_endpoint(
                    modules,
                    runtime_catalog,
                    &module.name,
                    &symbol.target,
                )?;
                RuntimeGuestImportResolution::RuntimeExport { endpoint }
            } else {
                let requested_image = modules
                    .images()
                    .image_by_name(&module.name)
                    .map_err(runtime_module_image_error)?;
                if requested_image.module_id != requested_node.id {
                    return Err(RuntimeStateError::new(
                        "module graph and mapped image registry disagree on the requested module",
                    ));
                }
                let lookup = match &symbol.target {
                    ImportTarget::Name { name, .. } => ExportLookup::Name { name: name.clone() },
                    ImportTarget::Ordinal { ordinal } => ExportLookup::Ordinal {
                        ordinal: u32::from(*ordinal),
                    },
                };
                let export = modules
                    .resolve(&module.name, lookup)
                    .map_err(runtime_module_image_error)?
                    .ok_or_else(|| {
                        RuntimeStateError::new(format!(
                            "required guest export in {} is absent",
                            module.name
                        ))
                    })?;
                for (requester, dependency) in export.dependencies {
                    dependency_edges.insert(RuntimeModuleDependencyEdge {
                        requester,
                        dependency,
                    });
                }
                RuntimeGuestImportResolution::GuestExport {
                    address: export.address,
                }
            };
            imports.push(RuntimeGuestImport {
                requested_module: requested_node.canonical_name.clone(),
                target: symbol.target.clone(),
                iat_address,
                resolution,
            });
        }
    }

    let binding = RuntimeGuestImportBinding {
        requester_module_id: requester_id,
        requester_handle,
        image_owner: image.layout.owner,
        imports,
    };
    let import_address_allocation = if needs_runtime_export_table
        && modules.runtime_system_exports().is_none()
    {
        Some(address_space.allocation_for_purpose(&ProcessAddressAllocationPurpose::ImportThunks)?)
    } else {
        None
    };
    let committed_bytes = if let Some(allocation) = import_address_allocation {
        validate_runtime_export_mapping_allocation(allocation)?;
        let bytes = runtime_export_committed_bytes(runtime_catalog)?;
        memory
            .preflight_growth(1, 1, bytes / GUEST_PAGE_SIZE as usize)
            .map_err(RuntimeStateError::from_memory)?;
        memory
            .preflight_fixed_reservation(IMPORT_THUNK_ARENA_BASE, IMPORT_THUNK_ARENA_SIZE)
            .map_err(RuntimeStateError::from_memory)?;
        bytes
    } else {
        0
    };

    let runtime_system_exports = if let Some(allocation) = import_address_allocation {
        Some(install_runtime_export_table_in_memory(
            memory,
            allocation,
            runtime_catalog,
            committed_bytes,
        )?)
    } else {
        None
    };
    for import in &binding.imports {
        memory
            .write_initialization(
                image.layout.owner,
                import.iat_address,
                &import.resolved_address().0.to_le_bytes(),
            )
            .map_err(RuntimeStateError::from_memory)?;
    }
    memory
        .seal_owner(image.layout.owner)
        .map_err(RuntimeStateError::from_memory)?;
    let binding = modules
        .register_guest_import_binding(
            binding,
            &dependency_edges.into_iter().collect::<Vec<_>>(),
            runtime_system_exports,
        )?
        .clone();
    Ok(binding)
}

fn planned_runtime_export_endpoint(
    modules: &crate::runtime::RuntimeModules,
    catalog: &RuntimeExportCatalog,
    module: &str,
    target: &ImportTarget,
) -> Result<RuntimeSystemExportEndpoint, RuntimeStateError> {
    let (index, export) = catalog.resolve_with_index(module, target).ok_or_else(|| {
        RuntimeStateError::new(format!(
            "runtime export catalog does not declare {module}!{target:?}"
        ))
    })?;
    let endpoint = runtime_export_endpoint(index, export.clone())?;
    if let Some(table) = modules.runtime_system_exports() {
        validate_runtime_export_table_catalog(table, catalog)?;
        let installed = table.endpoints.get(index).ok_or_else(|| {
            RuntimeStateError::new("installed runtime export table is missing a catalog endpoint")
        })?;
        if installed != &endpoint {
            return Err(RuntimeStateError::new(
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
        return Err(RuntimeStateError::new(
            "runtime export catalog changed after its process thunk table was installed",
        ));
    }
    for (index, export) in catalog.exports().iter().enumerate() {
        if table.endpoints.get(index) != Some(&runtime_export_endpoint(index, export.clone())?) {
            return Err(RuntimeStateError::new(
                "runtime export catalog order or identity changed after installation",
            ));
        }
    }
    Ok(())
}

pub(crate) fn runtime_export_endpoint(
    index: usize,
    export: crate::imports::RuntimeExport,
) -> Result<RuntimeSystemExportEndpoint, RuntimeStateError> {
    if index >= IMPORT_THUNK_CAPACITY {
        return Err(RuntimeStateError::new(
            "runtime export catalog exceeds the process import-thunk arena capacity",
        ));
    }
    let id =
        ImportThunkId(u32::try_from(index + 1).map_err(|_| {
            RuntimeStateError::new("runtime export endpoint ID does not fit 32 bits")
        })?);
    let offset = u32::try_from(index)
        .ok()
        .and_then(|index| index.checked_mul(IMPORT_THUNK_SLOT_SIZE))
        .ok_or_else(|| {
            RuntimeStateError::new("runtime export endpoint offset overflowed 32-bit accounting")
        })?;
    let address = IMPORT_THUNK_ARENA_BASE
        .0
        .checked_add(offset)
        .map(GuestAddress)
        .ok_or_else(|| {
            RuntimeStateError::new("runtime export endpoint address overflowed x86 accounting")
        })?;
    Ok(RuntimeSystemExportEndpoint {
        id,
        address,
        export,
    })
}

fn install_runtime_export_table_in_memory(
    memory: &mut GuestMemory,
    address_allocation: &ProcessAddressAllocationRecord,
    catalog: &RuntimeExportCatalog,
    committed_bytes: usize,
) -> Result<RuntimeSystemExportTable, RuntimeStateError> {
    let owner = memory
        .create_owner(MemoryOwnerKind::Runtime)
        .map_err(RuntimeStateError::from_memory)?;
    memory
        .reserve_fixed_for_address_allocation(
            address_allocation.id.0,
            owner,
            MemoryRegionKind::Runtime,
            IMPORT_THUNK_ARENA_BASE,
            IMPORT_THUNK_ARENA_SIZE,
        )
        .map_err(RuntimeStateError::from_memory)?;
    let committed_range = GuestRange::new(
        IMPORT_THUNK_ARENA_BASE,
        u64::try_from(committed_bytes).map_err(|_| {
            RuntimeStateError::new(
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
    memory
        .seal_owner(owner)
        .map_err(RuntimeStateError::from_memory)?;
    Ok(RuntimeSystemExportTable { endpoints })
}

fn validate_runtime_export_mapping_allocation(
    allocation: &ProcessAddressAllocationRecord,
) -> Result<(), RuntimeStateError> {
    validate_process_address_mapping_allocation(
        allocation,
        &ProcessAddressMappingRequirements {
            purpose: ProcessAddressAllocationPurpose::ImportThunks,

            range: RuntimeAddressRange {
                base: IMPORT_THUNK_ARENA_BASE,
                byte_length: IMPORT_THUNK_ARENA_SIZE,
            },
            classification: ProcessAddressAllocationClassification::RuntimeControl,

            release_policy: ProcessAddressAllocationReleasePolicy::Permanent,
            executable: true,
        },
    )
    .map_err(RuntimeStateError::from_memory)
}

fn runtime_export_committed_bytes(
    catalog: &RuntimeExportCatalog,
) -> Result<usize, RuntimeStateError> {
    if catalog.is_empty() {
        return Err(RuntimeStateError::new(
            "a runtime-system import requires at least one declared runtime export",
        ));
    }
    if catalog.len() > IMPORT_THUNK_CAPACITY {
        return Err(RuntimeStateError::new(
            "runtime export catalog exceeds the process import-thunk arena capacity",
        ));
    }
    Ok(IMPORT_THUNK_ARENA_SIZE as usize)
}

fn fill_runtime_export_table(
    memory: &mut GuestMemory,
    owner: crate::memory::MemoryOwnerId,
    byte_length: usize,
) -> Result<(), RuntimeStateError> {
    let chunk_limit = memory.limits().max_access_bytes;
    let mut halt_bytes = vec![0xf4; byte_length];
    for slot in halt_bytes.chunks_exact_mut(IMPORT_THUNK_SLOT_SIZE as usize) {
        slot[..2].copy_from_slice(&[0x0f, 0x0b]);
    }
    let mut offset = 0usize;
    while offset < byte_length {
        let chunk_length = (byte_length - offset).min(chunk_limit);
        let offset_u32 = u32::try_from(offset).map_err(|_| {
            RuntimeStateError::new("runtime export table fill offset does not fit 32 bits")
        })?;
        let address = IMPORT_THUNK_ARENA_BASE
            .0
            .checked_add(offset_u32)
            .map(GuestAddress)
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "runtime export table fill address overflowed x86 accounting",
                )
            })?;
        memory
            .write_initialization(owner, address, &halt_bytes[offset..offset + chunk_length])
            .map_err(RuntimeStateError::from_memory)?;
        offset = offset.checked_add(chunk_length).ok_or_else(|| {
            RuntimeStateError::new("runtime export table fill offset overflowed host accounting")
        })?;
    }
    Ok(())
}

fn validate_requester(
    memory: &GuestMemory,
    modules: &crate::runtime::RuntimeModules,
    requester_handle: u32,
    loader: &Pe32LoadPlan,
    image: &ImageMaterialization,
) -> Result<(), RuntimeStateError> {
    modules.module(requester_handle)?;
    let node = modules
        .graph()
        .module_by_handle(requester_handle)
        .map_err(runtime_module_graph_error)?;
    let expected_kind = match node.class {
        RuntimeModuleClass::MainExecutable => ImageKind::Executable,
        RuntimeModuleClass::GuestDynamicLibrary => ImageKind::DynamicLibrary,
        RuntimeModuleClass::RuntimeSystemModule => {
            return Err(RuntimeStateError::new(
                "runtime system modules do not own guest PE import tables",
            ));
        }
    };
    let expected_entry_point = image
        .layout
        .image_range
        .start()
        .0
        .checked_add(loader.image.entry_point_rva.0)
        .map(GuestAddress)
        .ok_or_else(|| {
            RuntimeStateError::new("requester load base plus entry-point RVA overflowed")
        })?;
    if loader.source != image.source
        || loader.image.kind != expected_kind
        || image.layout.image_range.byte_length() != u64::from(loader.image.size_of_image)
        || image.entry_point_address != expected_entry_point
    {
        return Err(RuntimeStateError::new(
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
        .ok_or_else(|| RuntimeStateError::new("requester load base plus IAT RVA overflowed"))
}

fn read_initialization_u32(
    memory: &GuestMemory,
    owner: crate::memory::MemoryOwnerId,
    address: GuestAddress,
) -> Result<u32, RuntimeStateError> {
    let bytes = memory
        .read_initialization(owner, address, 4)
        .map_err(RuntimeStateError::from_memory)?;
    let bytes: [u8; 4] = bytes
        .try_into()
        .map_err(|_| RuntimeStateError::new("preflighted IAT read did not return four bytes"))?;
    Ok(u32::from_le_bytes(bytes))
}

fn runtime_module_graph_error(
    error: crate::module_graph::RuntimeModuleGraphError,
) -> RuntimeStateError {
    RuntimeStateError::new(error.to_string())
}

fn runtime_module_image_error(error: RuntimeModuleImageError) -> RuntimeStateError {
    RuntimeStateError::new(error.to_string())
}
