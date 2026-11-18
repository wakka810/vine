use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::materialize::ImageMaterialization;
use crate::memory::{GuestAddress, GuestMemory, GuestMemoryError, GuestRange, MemoryOwnerId};
use crate::module_graph::{
    RuntimeModuleClass, RuntimeModuleGraphError, RuntimeModuleId, RuntimeModuleLoadState,
    RuntimeModuleNode,
};
use crate::pe32::{
    ExportDirectoryPlan, ExportLookup, ExportSymbolPlan, ExportTarget, ImageKind, Pe32LoadPlan,
    ResourceDirectoryPlan,
};

pub const RUNTIME_MODULE_IMAGE_LIMIT: usize = 4_096;
pub const RUNTIME_EXPORT_FORWARDER_DEPTH_LIMIT: usize = 32;
pub const RUNTIME_EXPORT_LOOKUP_NAME_LIMIT: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeModuleImage {
    pub module_id: RuntimeModuleId,
    pub handle: u32,
    pub canonical_name: String,
    pub memory_owner: MemoryOwnerId,
    pub image_range: GuestRange,
    pub entry_point: Option<GuestAddress>,
    pub(crate) exports: Option<ExportDirectoryPlan>,
    pub(crate) resources: Option<ResourceDirectoryPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeResolvedExport {
    pub address: GuestAddress,
    pub dependencies: Vec<(RuntimeModuleId, RuntimeModuleId)>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct RuntimeModuleImageRegistry {
    images: BTreeMap<RuntimeModuleId, RuntimeModuleImage>,
    canonical_index: BTreeMap<String, RuntimeModuleId>,
    handle_index: BTreeMap<u32, RuntimeModuleId>,
    load_base_index: BTreeMap<GuestAddress, RuntimeModuleId>,
}

impl RuntimeModuleImageRegistry {
    pub(crate) fn remove(&mut self, id: RuntimeModuleId) {
        if let Some(image) = self.images.remove(&id) {
            self.canonical_index.remove(&image.canonical_name);
            self.handle_index.remove(&image.handle);
            self.load_base_index.remove(&image.image_range.start());
        }
    }

    pub fn register(
        &mut self,
        memory: &GuestMemory,
        module: &RuntimeModuleNode,
        loader: &Pe32LoadPlan,
        materialization: &ImageMaterialization,
    ) -> Result<&RuntimeModuleImage, RuntimeModuleImageError> {
        if self.images.len() >= RUNTIME_MODULE_IMAGE_LIMIT {
            return Err(RuntimeModuleImageError::new(format!(
                "mapped module image limit {} was reached",
                RUNTIME_MODULE_IMAGE_LIMIT
            )));
        }
        if module.state != RuntimeModuleLoadState::Registered {
            return Err(RuntimeModuleImageError::new(
                "module image registration requires a registered graph node",
            ));
        }
        let expected_kind = match module.class {
            RuntimeModuleClass::MainExecutable => ImageKind::Executable,
            RuntimeModuleClass::GuestDynamicLibrary => ImageKind::DynamicLibrary,
            RuntimeModuleClass::RuntimeSystemModule => {
                return Err(RuntimeModuleImageError::new(
                    "runtime system modules do not own guest PE images",
                ));
            }
        };
        if loader.image.kind != expected_kind {
            return Err(RuntimeModuleImageError::new(
                "module graph class and PE image kind disagree",
            ));
        }
        let expected_entry_point = materialization
            .layout
            .image_range
            .start()
            .0
            .checked_add(loader.image.entry_point_rva.0)
            .map(GuestAddress)
            .ok_or_else(|| {
                RuntimeModuleImageError::new(
                    "load base plus PE entry-point RVA overflowed x86 accounting",
                )
            })?;
        if loader.source != materialization.source
            || materialization.layout.image_range.byte_length()
                != u64::from(loader.image.size_of_image)
            || materialization.entry_point_address != expected_entry_point
        {
            return Err(RuntimeModuleImageError::new(
                "loader plan and image materialization identities disagree",
            ));
        }
        memory
            .validate_sealed_image_owner(
                materialization.layout.owner,
                &materialization.source,
                materialization.layout.image_range,
            )
            .map_err(RuntimeModuleImageError::from_memory)?;
        // The PE export directory name is metadata supplied by the original linker; renaming
        // an executable or DLL does not change it. The selected filename owns module identity.
        if self.images.contains_key(&module.id)
            || self.canonical_index.contains_key(&module.canonical_name)
            || self.handle_index.contains_key(&module.handle)
        {
            return Err(RuntimeModuleImageError::new(
                "module ID, canonical name, or handle already owns a mapped image",
            ));
        }
        if self
            .images
            .values()
            .any(|image| ranges_overlap(image.image_range, materialization.layout.image_range))
        {
            return Err(RuntimeModuleImageError::new(
                "mapped module image range overlaps another registered image",
            ));
        }
        validate_export_addresses(&loader.exports, materialization)?;
        validate_resource_addresses(&loader.resources, materialization)?;

        let image = RuntimeModuleImage {
            module_id: module.id,
            handle: module.handle,
            canonical_name: module.canonical_name.clone(),
            memory_owner: materialization.layout.owner,
            image_range: materialization.layout.image_range,
            entry_point: (loader.image.entry_point_rva.0 != 0).then_some(expected_entry_point),
            exports: loader.exports.clone(),
            resources: loader.resources.clone(),
        };
        let module_id = module.id;
        let canonical_name = module.canonical_name.clone();
        let handle = module.handle;
        self.canonical_index.insert(canonical_name, module_id);
        self.handle_index.insert(handle, module_id);
        let previous_load_base = self
            .load_base_index
            .insert(materialization.layout.image_range.start(), module_id);
        debug_assert!(previous_load_base.is_none());
        Ok(self.images.entry(module_id).or_insert(image))
    }

    pub fn image(
        &self,
        module_id: RuntimeModuleId,
    ) -> Result<&RuntimeModuleImage, RuntimeModuleImageError> {
        self.images.get(&module_id).ok_or_else(|| {
            RuntimeModuleImageError::new(format!("module ID {} has no mapped image", module_id.0))
        })
    }

    pub fn image_by_name(
        &self,
        name: &str,
    ) -> Result<&RuntimeModuleImage, RuntimeModuleImageError> {
        let canonical = normalize_module_identity(name, false)?;
        let module_id = self
            .canonical_index
            .get(&canonical)
            .copied()
            .ok_or_else(|| {
                RuntimeModuleImageError::new(format!("module {canonical} has no mapped image"))
            })?;
        self.image(module_id)
    }

    pub(crate) fn image_by_load_base(
        &self,
        load_base: GuestAddress,
    ) -> Result<Option<&RuntimeModuleImage>, RuntimeModuleImageError> {
        let Some(module_id) = self.load_base_index.get(&load_base) else {
            return Ok(None);
        };
        self.images.get(module_id).map(Some).ok_or_else(|| {
            RuntimeModuleImageError::new(
                "mapped image load-base index does not resolve to a live image",
            )
        })
    }

    pub fn images(&self) -> &BTreeMap<RuntimeModuleId, RuntimeModuleImage> {
        &self.images
    }

    pub fn resolve(
        &self,
        module: &str,
        symbol: ExportLookup,
    ) -> Result<Option<RuntimeResolvedExport>, RuntimeModuleImageError> {
        let mut current_module = normalize_module_identity(module, false)?;
        let mut current_symbol = symbol;
        let mut visited = BTreeSet::new();
        let mut dependencies = Vec::new();

        loop {
            validate_export_lookup(&current_symbol)?;
            if !visited.insert((current_module.clone(), current_symbol.clone())) {
                return Err(RuntimeModuleImageError::new(format!(
                    "export forwarder cycle revisits {}!{}",
                    current_module,
                    export_lookup_label(&current_symbol)
                )));
            }
            let Some(module_id) = self.canonical_index.get(&current_module) else {
                return Ok(None);
            };
            let image = self.images.get(module_id).ok_or_else(|| {
                RuntimeModuleImageError::new("mapped module name references a missing image")
            })?;
            if image.module_id != *module_id || image.canonical_name != current_module {
                return Err(RuntimeModuleImageError::new(
                    "mapped module name and live image identity disagree",
                ));
            }
            let Some(exports) = image.exports.as_ref() else {
                return Ok(None);
            };
            let Some(export) = find_export(exports, &current_symbol) else {
                return Ok(None);
            };
            match &export.target {
                ExportTarget::Address { rva } => {
                    let address = image
                        .image_range
                        .start()
                        .0
                        .checked_add(rva.0)
                        .map(GuestAddress)
                        .ok_or_else(|| {
                            RuntimeModuleImageError::new(
                                "mapped export address overflowed x86 accounting",
                            )
                        })?;
                    if !contains_address(image.image_range, address) {
                        return Err(RuntimeModuleImageError::new(
                            "mapped export address is outside its registered image",
                        ));
                    }
                    return Ok(Some(RuntimeResolvedExport {
                        address,
                        dependencies,
                    }));
                }
                ExportTarget::Forwarder { module, symbol, .. } => {
                    if dependencies.len() >= RUNTIME_EXPORT_FORWARDER_DEPTH_LIMIT {
                        return Err(RuntimeModuleImageError::new(format!(
                            "export forwarder chain exceeds {} hops",
                            RUNTIME_EXPORT_FORWARDER_DEPTH_LIMIT
                        )));
                    }
                    let target_module = normalize_module_identity(module, true)?;
                    let Some(target_id) = self.canonical_index.get(&target_module) else {
                        return Ok(None);
                    };
                    dependencies.push((image.module_id, *target_id));
                    current_module = target_module;
                    current_symbol = symbol.clone();
                }
            }
        }
    }
}

fn validate_export_addresses(
    exports: &Option<ExportDirectoryPlan>,
    materialization: &ImageMaterialization,
) -> Result<(), RuntimeModuleImageError> {
    let Some(exports) = exports else {
        return Ok(());
    };
    for export in &exports.symbols {
        if let ExportTarget::Address { rva } = &export.target {
            let address = materialization
                .layout
                .image_range
                .start()
                .0
                .checked_add(rva.0)
                .map(GuestAddress)
                .ok_or_else(|| {
                    RuntimeModuleImageError::new(
                        "load base plus export RVA overflowed x86 accounting",
                    )
                })?;
            if !contains_address(materialization.layout.image_range, address) {
                return Err(RuntimeModuleImageError::new(
                    "direct export address is outside the materialized image",
                ));
            }
        }
    }
    Ok(())
}

fn validate_resource_addresses(
    resources: &Option<ResourceDirectoryPlan>,
    materialization: &ImageMaterialization,
) -> Result<(), RuntimeModuleImageError> {
    let Some(resources) = resources else {
        return Ok(());
    };
    for entry in &resources.entries {
        if u64::from(entry.descriptor_rva.0) + 16 > materialization.layout.image_range.byte_length()
        {
            return Err(RuntimeModuleImageError::new(
                "resource descriptor is outside its materialized image",
            ));
        }
        let start = materialization
            .layout
            .image_range
            .start()
            .0
            .checked_add(entry.data_rva.0)
            .ok_or_else(|| {
                RuntimeModuleImageError::new(
                    "load base plus resource data RVA overflowed x86 accounting",
                )
            })?;
        let end = u64::from(start) + u64::from(entry.size);
        if start < materialization.layout.image_range.start().0
            || end > materialization.layout.image_range.end()
        {
            return Err(RuntimeModuleImageError::new(
                "resource data range is outside its materialized image",
            ));
        }
    }
    Ok(())
}

fn find_export<'a>(
    exports: &'a ExportDirectoryPlan,
    lookup: &ExportLookup,
) -> Option<&'a ExportSymbolPlan> {
    match lookup {
        ExportLookup::Name { name } => exports.symbol_by_name(name),
        ExportLookup::Ordinal { ordinal } => exports.symbol_by_ordinal(*ordinal),
    }
}

fn normalize_module_identity(
    name: &str,
    append_dll_extension: bool,
) -> Result<String, RuntimeModuleImageError> {
    let mut canonical = crate::module_graph::normalize_module_lookup(name).ok_or_else(|| {
        RuntimeModuleImageError::new(
            "mapped module identity must be a bounded Windows-932 basename",
        )
    })?;
    if append_dll_extension && !canonical.ends_with(".dll") {
        canonical.push_str(".dll");
    }
    if !canonical.ends_with(".dll") && !canonical.ends_with(".exe") {
        return Err(RuntimeModuleImageError::new(
            "mapped module identity must end in .dll or .exe",
        ));
    }
    Ok(canonical)
}

#[test]
fn module_identity_accepts_cp932_basenames_without_accepting_paths() {
    for name in ["試験用.EXE", "表.dll"] {
        assert_eq!(
            normalize_module_identity(name, false).unwrap(),
            name.to_ascii_lowercase()
        );
    }
    for name in ["..\\表.dll", "C:表.dll", "a/b.dll", "bad\0.exe", "😀.exe"] {
        assert!(normalize_module_identity(name, false).is_err());
    }
}

fn export_lookup_label(lookup: &ExportLookup) -> String {
    match lookup {
        ExportLookup::Name { name } => name.clone(),
        ExportLookup::Ordinal { ordinal } => format!("#{ordinal}"),
    }
}

fn validate_export_lookup(lookup: &ExportLookup) -> Result<(), RuntimeModuleImageError> {
    match lookup {
        ExportLookup::Name { name }
            if name.is_empty()
                || name.len() > RUNTIME_EXPORT_LOOKUP_NAME_LIMIT
                || !name.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) =>
        {
            Err(RuntimeModuleImageError::new(format!(
                "export lookup name must be printable ASCII within {} bytes",
                RUNTIME_EXPORT_LOOKUP_NAME_LIMIT
            )))
        }
        ExportLookup::Ordinal { ordinal } if *ordinal > u32::from(u16::MAX) => {
            Err(RuntimeModuleImageError::new(
                "export lookup ordinal exceeds the PE32 16-bit lookup range",
            ))
        }
        _ => Ok(()),
    }
}

fn ranges_overlap(left: GuestRange, right: GuestRange) -> bool {
    u64::from(left.start().0) < right.end() && u64::from(right.start().0) < left.end()
}

fn contains_address(range: GuestRange, address: GuestAddress) -> bool {
    let address = u64::from(address.0);
    address >= u64::from(range.start().0) && address < range.end()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeModuleImageError(String);

impl RuntimeModuleImageError {
    pub(crate) fn from_module_graph(error: RuntimeModuleGraphError) -> Self {
        Self(error.to_string())
    }

    fn from_memory(error: GuestMemoryError) -> Self {
        Self(error.to_string())
    }

    pub(crate) fn diagnostic(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for RuntimeModuleImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for RuntimeModuleImageError {}
