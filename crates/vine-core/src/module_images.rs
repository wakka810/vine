use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Serialize;

use crate::materialize::ImageMaterialization;
use crate::memory::{
    GuestAddress, GuestMemory, GuestMemoryError, GuestMemoryErrorClass, GuestRange, MemoryOwnerId,
};
use crate::module_graph::{
    RuntimeModuleClass, RuntimeModuleGraphError, RuntimeModuleGraphErrorClass, RuntimeModuleId,
    RuntimeModuleLoadState, RuntimeModuleNode,
};
use crate::pe32::{
    ExportDirectoryPlan, ExportLookup, ExportSymbolPlan, ExportTarget, ImageKind,
    LoaderIngressReport,
};
use crate::snapshot::ModuleIdentity;

pub const RUNTIME_MODULE_IMAGE_LIMIT: usize = 4_096;
pub const RUNTIME_EXPORT_FORWARDER_DEPTH_LIMIT: usize = 32;
pub const RUNTIME_EXPORT_LOOKUP_NAME_LIMIT: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeModuleImage {
    pub module_id: RuntimeModuleId,
    pub handle: u32,
    pub canonical_name: String,
    pub source: ModuleIdentity,
    pub memory_owner: MemoryOwnerId,
    pub image_range: GuestRange,
    pub preferred_image_base: GuestAddress,
    pub load_base: GuestAddress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_point: Option<GuestAddress>,
    pub mapped_content_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exports: Option<ExportDirectoryPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeExportForwarderHop {
    pub source_module_id: RuntimeModuleId,
    pub source_module: String,
    pub source_ordinal: u32,
    pub raw: String,
    pub target_module: String,
    pub target_symbol: ExportLookup,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeResolvedExport {
    pub requested_module: String,
    pub requested_symbol: ExportLookup,
    pub defining_module_id: RuntimeModuleId,
    pub defining_module: String,
    pub ordinal: u32,
    pub names: Vec<String>,
    pub address: GuestAddress,
    pub forwarders: Vec<RuntimeExportForwarderHop>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeModuleImageRegistry {
    images: BTreeMap<RuntimeModuleId, RuntimeModuleImage>,
    #[serde(skip)]
    canonical_index: BTreeMap<String, RuntimeModuleId>,
    #[serde(skip)]
    handle_index: BTreeMap<u32, RuntimeModuleId>,
}

impl RuntimeModuleImageRegistry {
    pub fn register(
        &mut self,
        memory: &GuestMemory,
        module: &RuntimeModuleNode,
        loader: &LoaderIngressReport,
        materialization: &ImageMaterialization,
    ) -> Result<&RuntimeModuleImage, RuntimeModuleImageError> {
        if self.images.len() >= RUNTIME_MODULE_IMAGE_LIMIT {
            return Err(RuntimeModuleImageError::resource_limit(
                "module-image-limit-exceeded",
                format!(
                    "mapped module image limit {} was reached",
                    RUNTIME_MODULE_IMAGE_LIMIT
                ),
            ));
        }
        if module.state != RuntimeModuleLoadState::Registered {
            return Err(RuntimeModuleImageError::conflict(
                "module-image-registration-state-invalid",
                "module image registration requires a registered graph node",
            ));
        }
        let expected_kind = match module.class {
            RuntimeModuleClass::MainExecutable => ImageKind::Executable,
            RuntimeModuleClass::GuestDynamicLibrary => ImageKind::DynamicLibrary,
            RuntimeModuleClass::RuntimeSystemModule => {
                return Err(RuntimeModuleImageError::invalid_input(
                    "runtime-system-module-image-forbidden",
                    "runtime system modules do not own guest PE images",
                ));
            }
        };
        if loader.load_plan.image.kind != expected_kind {
            return Err(RuntimeModuleImageError::invalid_input(
                "module-image-kind-mismatch",
                "module graph class and PE image kind disagree",
            ));
        }
        let expected_entry_point = materialization
            .load_base
            .0
            .checked_add(loader.load_plan.image.entry_point_rva.0)
            .map(GuestAddress)
            .ok_or_else(|| {
                RuntimeModuleImageError::invalid_input(
                    "module-image-entry-point-overflow",
                    "load base plus PE entry-point RVA overflowed x86 accounting",
                )
            })?;
        if loader.source != materialization.source
            || materialization.layout.source != materialization.source
            || materialization.preferred_image_base.0 != loader.load_plan.image.preferred_image_base
            || materialization.load_base != materialization.layout.image_range.start()
            || materialization.layout.image_range.byte_length()
                != u64::from(loader.load_plan.image.size_of_image)
            || materialization.relocation_delta
                != materialization
                    .load_base
                    .0
                    .wrapping_sub(loader.load_plan.image.preferred_image_base)
            || materialization.entry_point_address != expected_entry_point
        {
            return Err(RuntimeModuleImageError::invalid_input(
                "module-image-materialization-mismatch",
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
        if let Some(exports) = &loader.load_plan.exports {
            let export_module = normalize_module_identity(&exports.module_name, false)?;
            if export_module != module.canonical_name {
                return Err(RuntimeModuleImageError::invalid_input(
                    "export-module-identity-mismatch",
                    format!(
                        "export directory identifies {export_module}, but graph node is {}",
                        module.canonical_name
                    ),
                ));
            }
        }
        if self.images.contains_key(&module.id)
            || self.canonical_index.contains_key(&module.canonical_name)
            || self.handle_index.contains_key(&module.handle)
        {
            return Err(RuntimeModuleImageError::conflict(
                "module-image-identity-duplicate",
                "module ID, canonical name, or handle already owns a mapped image",
            ));
        }
        if self
            .images
            .values()
            .any(|image| ranges_overlap(image.image_range, materialization.layout.image_range))
        {
            return Err(RuntimeModuleImageError::conflict(
                "module-image-range-overlap",
                "mapped module image range overlaps another registered image",
            ));
        }
        validate_export_addresses(&loader.load_plan.exports, materialization)?;

        let image = RuntimeModuleImage {
            module_id: module.id,
            handle: module.handle,
            canonical_name: module.canonical_name.clone(),
            source: materialization.source.clone(),
            memory_owner: materialization.layout.owner,
            image_range: materialization.layout.image_range,
            preferred_image_base: materialization.preferred_image_base,
            load_base: materialization.load_base,
            entry_point: (loader.load_plan.image.entry_point_rva.0 != 0)
                .then_some(expected_entry_point),
            mapped_content_sha256: materialization.mapped_content_sha256.clone(),
            exports: loader.load_plan.exports.clone(),
        };
        let module_id = module.id;
        let canonical_name = module.canonical_name.clone();
        let handle = module.handle;
        self.canonical_index.insert(canonical_name, module_id);
        self.handle_index.insert(handle, module_id);
        Ok(self.images.entry(module_id).or_insert(image))
    }

    pub fn image(
        &self,
        module_id: RuntimeModuleId,
    ) -> Result<&RuntimeModuleImage, RuntimeModuleImageError> {
        self.images.get(&module_id).ok_or_else(|| {
            RuntimeModuleImageError::not_found(
                "module-image-id-unknown",
                format!("module ID {} has no mapped image", module_id.0),
            )
        })
    }

    pub fn image_by_handle(
        &self,
        handle: u32,
    ) -> Result<&RuntimeModuleImage, RuntimeModuleImageError> {
        let module_id = self.handle_index.get(&handle).copied().ok_or_else(|| {
            RuntimeModuleImageError::not_found(
                "module-image-handle-unknown",
                format!("module handle {handle:#010x} has no mapped image"),
            )
        })?;
        self.image(module_id)
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
                RuntimeModuleImageError::not_found(
                    "module-image-name-unknown",
                    format!("module {canonical} has no mapped image"),
                )
            })?;
        self.image(module_id)
    }

    pub fn images(&self) -> &BTreeMap<RuntimeModuleId, RuntimeModuleImage> {
        &self.images
    }

    pub fn resolve(
        &self,
        module: &str,
        symbol: ExportLookup,
    ) -> Result<RuntimeResolvedExport, RuntimeModuleImageError> {
        let requested_module = normalize_module_identity(module, false)?;
        let requested_symbol = symbol.clone();
        let mut current_module = requested_module.clone();
        let mut current_symbol = symbol;
        let mut visited = BTreeSet::new();
        let mut forwarders = Vec::new();

        loop {
            validate_export_lookup(&current_symbol)?;
            if !visited.insert((current_module.clone(), current_symbol.clone())) {
                return Err(RuntimeModuleImageError::conflict(
                    "export-forwarder-cycle",
                    format!(
                        "export forwarder cycle revisits {}!{}",
                        current_module,
                        export_lookup_label(&current_symbol)
                    ),
                ));
            }
            let image = self.image_by_name(&current_module).map_err(|error| {
                if !forwarders.is_empty() && error.code == "module-image-name-unknown" {
                    RuntimeModuleImageError::not_found(
                        "export-forwarder-target-module-unknown",
                        format!(
                            "export forwarder target module {current_module} has no mapped image"
                        ),
                    )
                } else {
                    error
                }
            })?;
            let exports = image.exports.as_ref().ok_or_else(|| {
                RuntimeModuleImageError::not_found(
                    "module-export-directory-missing",
                    format!("module {} has no export directory", image.canonical_name),
                )
            })?;
            let export = find_export(exports, &current_symbol).ok_or_else(|| {
                RuntimeModuleImageError::not_found(
                    "module-export-symbol-unknown",
                    format!(
                        "module {} does not export {}",
                        image.canonical_name,
                        export_lookup_label(&current_symbol)
                    ),
                )
            })?;
            match &export.target {
                ExportTarget::Address { rva } => {
                    let address = image
                        .load_base
                        .0
                        .checked_add(rva.0)
                        .map(GuestAddress)
                        .ok_or_else(|| {
                            RuntimeModuleImageError::runtime_defect(
                                "resolved-export-address-overflow",
                                "mapped export address overflowed x86 accounting",
                            )
                        })?;
                    if !contains_address(image.image_range, address) {
                        return Err(RuntimeModuleImageError::runtime_defect(
                            "resolved-export-outside-image",
                            "mapped export address is outside its registered image",
                        ));
                    }
                    return Ok(RuntimeResolvedExport {
                        requested_module,
                        requested_symbol,
                        defining_module_id: image.module_id,
                        defining_module: image.canonical_name.clone(),
                        ordinal: export.ordinal,
                        names: export.names.clone(),
                        address,
                        forwarders,
                    });
                }
                ExportTarget::Forwarder {
                    raw,
                    module,
                    symbol,
                } => {
                    if forwarders.len() >= RUNTIME_EXPORT_FORWARDER_DEPTH_LIMIT {
                        return Err(RuntimeModuleImageError::resource_limit(
                            "export-forwarder-depth-limit-exceeded",
                            format!(
                                "export forwarder chain exceeds {} hops",
                                RUNTIME_EXPORT_FORWARDER_DEPTH_LIMIT
                            ),
                        ));
                    }
                    let target_module = normalize_module_identity(module, true)?;
                    forwarders.push(RuntimeExportForwarderHop {
                        source_module_id: image.module_id,
                        source_module: image.canonical_name.clone(),
                        source_ordinal: export.ordinal,
                        raw: raw.clone(),
                        target_module: target_module.clone(),
                        target_symbol: symbol.clone(),
                    });
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
                .load_base
                .0
                .checked_add(rva.0)
                .map(GuestAddress)
                .ok_or_else(|| {
                    RuntimeModuleImageError::invalid_input(
                        "module-export-address-overflow",
                        "load base plus export RVA overflowed x86 accounting",
                    )
                })?;
            if !contains_address(materialization.layout.image_range, address) {
                return Err(RuntimeModuleImageError::invalid_input(
                    "module-export-outside-image",
                    "direct export address is outside the materialized image",
                ));
            }
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
    if name.is_empty()
        || name.len() > 260
        || !name.is_ascii()
        || name
            .bytes()
            .any(|byte| byte.is_ascii_control() || matches!(byte, b'/' | b'\\' | b':'))
    {
        return Err(RuntimeModuleImageError::invalid_input(
            "module-image-name-invalid",
            "mapped module identity must be a bounded ASCII basename",
        ));
    }
    let mut canonical = name.to_ascii_lowercase();
    if append_dll_extension && !canonical.ends_with(".dll") {
        canonical.push_str(".dll");
    }
    if !canonical.ends_with(".dll") && !canonical.ends_with(".exe") {
        return Err(RuntimeModuleImageError::invalid_input(
            "module-image-extension-invalid",
            "mapped module identity must end in .dll or .exe",
        ));
    }
    Ok(canonical)
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
            Err(RuntimeModuleImageError::invalid_input(
                "export-lookup-name-invalid",
                format!(
                    "export lookup name must be printable ASCII within {} bytes",
                    RUNTIME_EXPORT_LOOKUP_NAME_LIMIT
                ),
            ))
        }
        ExportLookup::Ordinal { ordinal } if *ordinal > u32::from(u16::MAX) => {
            Err(RuntimeModuleImageError::invalid_input(
                "export-lookup-ordinal-invalid",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeModuleImageErrorClass {
    InvalidInput,
    NotFound,
    Conflict,
    ResourceLimit,
    RuntimeDefect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeModuleImageError {
    pub class: RuntimeModuleImageErrorClass,
    pub code: &'static str,
    pub message: String,
}

impl RuntimeModuleImageError {
    pub(crate) fn from_module_graph(error: RuntimeModuleGraphError) -> Self {
        let class = match error.class {
            RuntimeModuleGraphErrorClass::InvalidInput => {
                RuntimeModuleImageErrorClass::InvalidInput
            }
            RuntimeModuleGraphErrorClass::NotFound => RuntimeModuleImageErrorClass::NotFound,
            RuntimeModuleGraphErrorClass::Conflict => RuntimeModuleImageErrorClass::Conflict,
            RuntimeModuleGraphErrorClass::ResourceLimit => {
                RuntimeModuleImageErrorClass::ResourceLimit
            }
            RuntimeModuleGraphErrorClass::RuntimeDefect => {
                RuntimeModuleImageErrorClass::RuntimeDefect
            }
        };
        Self::new(class, error.code, error.message)
    }

    fn invalid_input(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeModuleImageErrorClass::InvalidInput, code, message)
    }

    fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeModuleImageErrorClass::NotFound, code, message)
    }

    fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeModuleImageErrorClass::Conflict, code, message)
    }

    fn resource_limit(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeModuleImageErrorClass::ResourceLimit, code, message)
    }

    fn runtime_defect(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeModuleImageErrorClass::RuntimeDefect, code, message)
    }

    fn from_memory(error: GuestMemoryError) -> Self {
        let class = match error.class {
            GuestMemoryErrorClass::ResourceLimit => RuntimeModuleImageErrorClass::ResourceLimit,
            GuestMemoryErrorClass::RuntimeDefect => RuntimeModuleImageErrorClass::RuntimeDefect,
            GuestMemoryErrorClass::Conflict | GuestMemoryErrorClass::SynchronizationRequired => {
                RuntimeModuleImageErrorClass::Conflict
            }
            GuestMemoryErrorClass::InvalidRequest | GuestMemoryErrorClass::Fault => {
                RuntimeModuleImageErrorClass::InvalidInput
            }
        };
        Self::new(class, error.code, error.message)
    }

    fn new(
        class: RuntimeModuleImageErrorClass,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            class,
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for RuntimeModuleImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "module-image / {}: {}", self.code, self.message)
    }
}

impl std::error::Error for RuntimeModuleImageError {}
